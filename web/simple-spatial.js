'use strict';
// A view onto Design, never a second saved representation.
const SimpleSpatial = (() => {
  const names = {front:'正面で固定',left:'左耳側',right:'右耳側',approach:'正面から近づく',pass:'左から右へ通過',reversePass:'右から左へ通過',rearPass:'背後を回る',above:'頭上を通過'};
  function apply(d, id) {
    if (!names[id]) throw Error('不明なプリセットです');
    Spatial.applyPreset(d, id === 'reversePass' || id === 'rearPass' ? 'pass' : id);
    if (id === 'reversePass') Spatial.transform(d, 'reverse');
    if (id === 'rearPass') d.keyframes[1].azimuth = -180;
    return d;
  }
  function poses(d) { return d.motion === 'path' ? d.keyframes : [d]; }
  function move(d, azimuth, distance) {
    if (d.motion === 'sweep') {
      // Convert the legacy long arc exactly before rotating across the +/-180 seam.
      const steps=Math.max(1,Math.ceil(Math.abs(d.endAzimuth-d.azimuth)/90));
      d.keyframes=Array.from({length:steps+1},(_,i)=>Spatial.pose(d,i/steps));
      d.motion='path';
    }
    const first = Spatial.pose(d, 0), delta = Spatial.wrap(azimuth - first.azimuth);
    const all = poses(d), distances = all.map(p => p.distanceM);
    const scale = Math.max(.3 / Math.min(...distances), Math.min(5 / Math.max(...distances), distance / first.distanceM));
    all.forEach(p => { p.azimuth = Spatial.wrap(p.azimuth + delta); p.distanceM *= scale; });
    if (d.motion === 'sweep') d.endAzimuth = Spatial.wrap(d.endAzimuth + delta);
  }
  function identify(d) {
    // Exact rendered trajectories only: switching views never approximates or rewrites a path.
    for (const id of Object.keys(names)) {
      const candidate = apply(structuredClone(d), id);
      if (candidate.motion!==d.motion) continue;
      const knots=[...new Set([0,1,...(d.keyframes||[]).map(k=>k.at),...(candidate.keyframes||[]).map(k=>k.at)])].sort((a,b)=>a-b);
      const times=[...knots,...knots.slice(1).map((t,i)=>(t+knots[i])/2)];
      if (times.every(t => {
        const a = Spatial.pose(d,t), b = Spatial.pose(candidate,t);
        return Math.abs(Spatial.wrap(a.azimuth-b.azimuth)) < .0001 && Math.abs(a.elevation-b.elevation) < .0001 && Math.abs(a.distanceM-b.distanceM) < .0001;
      })) return id;
    }
    return '';
  }
  function distance(d, maximum) {
    const all=poses(d), scale=maximum/Math.max(...all.map(p=>p.distanceM));
    all.forEach(p=>p.distanceM=Math.max(.3,Math.min(5,p.distanceM*scale)));
  }
  return {names, apply, move, identify, distance};
})();
if (typeof module !== 'undefined') module.exports = SimpleSpatial;

let audioEditorMode = 'simple', audioAB = 'design';
function audioPreviewDesign(d) { const copy=structuredClone(d); if(audioAB!=='design')copy.spatial=audioAB==='spatial'; return copy; }
function simpleSpatialMarkup(d) {
  const selected=SimpleSpatial.identify(d);
  return `<section class="au-position au-simple"><div class="aw-row aw-between"><strong>音の位置と動き</strong><label><input id="audio-spatial" type="checkbox" ${d.spatial?'checked':''}> 立体音響 ${d.spatial?'オン':'オフ'}</label></div>
    <div class="au-mode aw-row"><button data-audio-mode="simple" aria-pressed="true">かんたん</button><button data-audio-mode="detail" aria-pressed="false">詳細</button></div>
    <div class="au-simple-layout"><div ${d.spatial?'':'inert'} class="${d.spatial?'':'au-disabled'}"><p>どこから</p><div id="simple-map-control" tabindex="0" role="group" aria-label="音源の開始位置。左右キーで向き、上下キーで距離を変更"><svg id="simple-map" viewBox="0 0 400 400" role="img" aria-label="頭を上から見た音源と移動方向"></svg></div><p class="aw-sub">音源の点をドラッグして開始位置を動かします。<br>矢印は進む向きです。</p></div>
    <div class="au-simple-choices"><div ${d.spatial?'':'inert'} class="${d.spatial?'':'au-disabled'}"><label>どう動く <select id="simple-preset"><option value="">${selected?'プリセットを選択':'カスタム · 現在の経路を保持'}</option>${Object.entries(SimpleSpatial.names).map(([id,name])=>`<option value="${id}" ${selected===id?'selected':''}>${name}</option>`).join('')}</select></label><p>どの距離</p><div class="aw-row"><button data-simple-distance="1">近く</button><button data-simple-distance="2.5">中間</button><button data-simple-distance="5">遠く</button></div></div>
    <div class="au-strength"><p>どの強さ</p><div class="aw-row"><button data-simple-level="-30">弱め</button><button data-simple-level="-24">標準</button><button data-simple-level="-18">強め</button><span id="simple-level" class="aw-sub"></span></div></div>
    <p class="aw-sub">${d.spatial?'イヤホンで試聴してください。距離と強さは実際の感じ方を確認して調整します。':'立体音響オフ · 元の左右を保持して再生します。設定した経路は保存されます。'}</p><p class="aw-sub">カスタム経路の細かい調整は「詳細」へ。プリセットを選ぶと経路を置き換えます。</p></div></div></section>`;
}
function bindSimpleSpatial() {
  $('#audio-spatial').onchange=()=>attempt(async()=>{audioDesign.spatial=$('#audio-spatial').checked;audioRevision++;await flushAudioDraft();audioEditor()});
  const commit=async f=>{f();audioRevision++;await flushAudioDraft();audioEditor()};
  $('#simple-preset').onchange=()=>{const id=$('#simple-preset').value;if(id)attempt(()=>commit(()=>SimpleSpatial.apply(audioDesign,id)))};
  $$('[data-simple-distance]').forEach(b=>b.onclick=()=>attempt(()=>commit(()=>SimpleSpatial.distance(audioDesign,+b.dataset.simpleDistance))));
  $$('[data-simple-level]').forEach(b=>b.onclick=()=>attempt(()=>commit(()=>{audioDesign.levelDb=+b.dataset.simpleLevel})));
  const map=$('#simple-map-control');
  const update=e=>{if(!audioPointerEditing)return;const r=map.getBoundingClientRect(),x=(e.clientX-r.left)/r.width*400-200,y=200-(e.clientY-r.top)/r.height*400;SimpleSpatial.move(audioDesign,Math.atan2(x,y)*180/Math.PI,Math.max(.3,Math.min(5,(Math.hypot(x,y)-40)/24)));queueAudioEdit();drawSimpleSpatial()};
  map.onpointerdown=e=>{if(!audioDesign.spatial)return;audioPointerEditing=true;map.setPointerCapture(e.pointerId);map.focus();update(e)};
  map.onpointermove=update;
  map.onpointerup=map.onpointercancel=map.onlostpointercapture=()=>{if(!audioPointerEditing)return;audioPointerEditing=false;attempt(flushAudioDraft)};
  map.onkeydown=e=>{if(!audioDesign.spatial||!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;e.preventDefault();const p=Spatial.pose(audioDesign,0);SimpleSpatial.move(audioDesign,p.azimuth+(e.key==='ArrowLeft'?-5:e.key==='ArrowRight'?5:0),p.distanceM+(e.key==='ArrowUp'?-.1:e.key==='ArrowDown'?.1:0));queueAudioEdit();drawSimpleSpatial()};
}
function drawSimpleSpatial() {
  if(!$('#simple-map')||!audioDesign)return;
  const d=audioDesign,xy=p=>{const a=p.azimuth*Math.PI/180,r=40+p.distanceM*24;return [200+Math.sin(a)*r,200-Math.cos(a)*r]};
  const start=xy(Spatial.pose(d,0)),path=Array.from({length:65},(_,i)=>(i?'L':'M')+xy(Spatial.pose(d,i/64)).join(' ')).join('');
  $('#simple-map').innerHTML=`<defs><marker id="simple-arrow" markerWidth="9" markerHeight="9" refX="7" refY="4" orient="auto"><path d="M0 0L8 4L0 8" fill="none" stroke="var(--aw-ink)"/></marker></defs><rect width="400" height="400" rx="12" fill="var(--aw-inset)"/>${[64,100,160].map(r=>`<circle cx="200" cy="200" r="${r}" fill="none" stroke="var(--aw-line)"/>`).join('')}<g fill="var(--aw-sub)" text-anchor="middle" font-size="14"><text x="200" y="22">前</text><text x="200" y="390">後ろ</text><text x="18" y="205">左</text><text x="382" y="205">右</text></g><circle cx="200" cy="200" r="27" fill="var(--aw-panel)" stroke="var(--aw-ink)"/><path d="M192 175L200 163L208 175M169 192V208M231 192V208" fill="none" stroke="var(--aw-ink)" stroke-width="3"/><path d="${path}" fill="none" stroke="var(--aw-ink)" stroke-width="3" ${d.motion==='fixed'?'':'marker-end="url(#simple-arrow)"'}/><circle cx="${start[0]}" cy="${start[1]}" r="12" fill="var(--aw-ink)"/><text x="${start[0]}" y="${start[1]-19}" text-anchor="middle" fill="var(--aw-ink)" font-size="13">音源</text>`;
  $('#simple-preset').value=SimpleSpatial.identify(d);
  $('#simple-preset').options[0].textContent=$('#simple-preset').value?'プリセットを選択':'カスタム · 現在の経路を保持';
  $('#simple-level').textContent=[-30,-24,-18].includes(d.levelDb)?'':'カスタム';
  $$('[data-simple-level]').forEach(b=>b.setAttribute('aria-pressed',+b.dataset.simpleLevel===d.levelDb));
}
