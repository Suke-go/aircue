'use strict';
let studyState={open:false},studyBusy=false,studyShape='';
const studyMeasures={strength:'強さ',naturalness:'自然さ',comfort:'快適さ'};
function studyMarkup(){return `<section class="ex-card"><h2>AirCueで実行</h2><p>参加者を準備し、左右と快適レベルを確認してから、1試行ずつ提示します。Unityや外部アプリは不要です。4ch出力を使用します。</p><div id="study-controls"></div><p id="study-status" role="status"></p><p class="aw-sub">回答ボタンを押すと主指標を先に記録します。補助評定はその後に入力します。反応時間はソフトウェア上の推定値で、音・噴流の実測開始時刻ではありません。</p></section>`;}
function bindStudy(){studyShape='';attempt(()=>refreshStudy());}
async function studyDo(action,value=null){if(studyBusy)return;studyBusy=true;try{studyState=await invoke('study_action',{action,value});studyShape='';await refreshStudy();}finally{studyBusy=false}}
async function refreshStudy(){
 if(!$('#study-controls'))return;
 const s=await invoke('study_status');if(!$('#study-controls'))return;studyState=s;
 $('#study-status').textContent=s.open?`${s.participant} · ${s.completed}/${s.total} 試行完了${s.block?' · ブロック '+s.block:''} · ${s.fault||(!s.calibrated?'左右と快適レベルを確認してください':s.active?(s.responded?'補助評定を入力し、再生終了後に試行を保存してください':'刺激を感じたら回答してください'):'次の試行を開始できます')} · 記録: ${s.logPath}`:'生成したmanifestから参加者を準備します。準備だけでは再生しません。';
 const shape=JSON.stringify([s.open,s.participant,s.completed,s.calibrated,s.active,s.responded,s.checked,s.fault,s.secondary,experimentManifest?.plan.participants]);if(shape===studyShape)return;studyShape=shape;
 const area=$('#study-controls');
 if(!s.open){area.innerHTML=`<div class="aw-row"><label>参加者<select id="study-participant">${(experimentManifest?.plan.participants||[]).map(p=>`<option>${esc(p)}</option>`).join('')}</select></label><button id="study-start" ${experimentManifest?'':'disabled'}>参加者を準備</button></div>`;$('#study-start').onclick=()=>attempt(async()=>{if(studyBusy||!experimentManifest)return;studyBusy=true;try{await invoke('start_study',{manifest:experimentManifest,participant:$('#study-participant').value});await refreshStudy();}finally{studyBusy=false}});return;}
 area.innerHTML=`${!s.calibrated&&!s.fault?`<div class="aw-row">${['イヤホン左','イヤホン右','空気刺激左','空気刺激右'].map((v,i)=>`<button data-study-cal="${i}">${s.checked&(1<<i)?'✓ ':''}${v}</button>`).join('')}</div><div class="aw-row">${['イヤホンの左右','空気刺激の左右','快適なレベル'].map((v,i)=>`<label><input type="checkbox" data-study-check="${i}">${v}を確認</label>`).join('')}<button id="study-confirm" ${s.checked===15?'':'disabled'}>確認を記録</button></div>`:''}
 ${s.calibrated&&!s.fault?`<div class="aw-row"><button id="study-next" class="aw-primary" ${s.active||s.completed===s.total?'disabled':''}>次の試行を再生</button>${s.active&&!s.responded?(s.primary==='direction'?[['left','左'],['right','右'],['front','前'],['back','後ろ'],['uncertain','わからない']]:[['detected','感じた']]).map(([v,n])=>`<button data-study-answer="${v}">${n}</button>`).join(''):''}</div>`:''}
 ${s.active&&s.responded&&!s.fault?`<div class="aw-row">${s.secondary.map(v=>`<label>${studyMeasures[v]}（0〜10）<input data-study-rating="${v}" type="number" min="0" max="10" step="1" required></label>`).join('')}<button id="study-finish">この試行を保存</button></div>`:''}
 <div class="aw-row">${s.active?'<label>中止理由<input id="study-reason" maxlength="200" value="participantRequested"></label><button id="study-abort">この試行を中止</button>':''}<button id="study-end">セッションを終了</button></div>`;
 $$('[data-study-cal]').forEach(b=>b.onclick=()=>attempt(()=>studyDo('calibration',+b.dataset.studyCal)));
 if($('#study-confirm'))$('#study-confirm').onclick=()=>attempt(()=>studyDo('confirm',$$('[data-study-check]').map(c=>c.checked)));
 if($('#study-next'))$('#study-next').onclick=()=>attempt(()=>studyDo('next'));
 $$('[data-study-answer]').forEach(b=>b.onclick=()=>attempt(()=>studyDo('response',b.dataset.studyAnswer)));
 if($('#study-finish'))$('#study-finish').onclick=()=>attempt(()=>{const ratings={};for(const input of $$('[data-study-rating]')){if(!input.checkValidity()||input.value==='')throw Error('補助評定を0〜10で入力してください');ratings[input.dataset.studyRating]=+input.value;}return studyDo('finish',ratings);});
 if($('#study-abort'))$('#study-abort').onclick=()=>attempt(()=>studyDo('abort',$('#study-reason').value));
 $('#study-end').onclick=()=>attempt(()=>studyDo('end'));
}
