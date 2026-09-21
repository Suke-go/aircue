'use strict';
let experimentManifest=null, experimentGeneration=0, experimentPlan={template:'modality',seed:42,repetitions:4,blocks:1,participants:['P001'],primary:'direction',secondary:[],conditions:[]};
const experimentTemplates={congruency:'左右整合性',onset:'音と空気刺激の開始時間差',modality:'単独刺激と複合刺激',intensity:'知覚強度'};
function experimentSetMarkup(){
 const p=experimentPlan;
 return `<section class="ex-card ex-plan"><h2>実験セット</h2><p>基準のタイムラインに音声1クリップと空気刺激1クリップを置きます。条件に合わせて左右固定の刺激を作成します。</p><p class="aw-sub">基準の経路・配置時刻は条件用に置き換え、元のタイムラインは保持します。音声は10秒以内。レベル・距離・クリップ補正を基準として引き継ぎます。</p>
 <div class="ex-fields"><label>研究テンプレート<select id="set-template">${Object.entries(experimentTemplates).map(([id,name])=>`<option value="${id}" ${id===p.template?'selected':''}>${name}</option>`).join('')}</select></label><label>条件ごとの反復<input id="set-repetitions" type="number" min="1" max="8" value="${p.repetitions}"></label><label>ブロック数<input id="set-blocks" type="number" min="1" max="16" value="${p.blocks}"></label></div>
 <div class="ex-fields"><label>匿名参加者ID（カンマ区切り）<input id="set-participants" value="${esc(p.participants.join(','))}" placeholder="P001,P002"></label><label>ランダム化シード<input id="set-seed" type="number" min="0" max="4294967295" value="${p.seed}"></label><label>主指標<select id="set-primary"><option value="direction" ${p.primary==='direction'?'selected':''}>方向判断</option><option value="reactionTime" ${p.primary==='reactionTime'?'selected':''}>反応時間</option></select></label></div>
 <div class="aw-row">補助指標（必要なものだけ）${[['strength','強さ'],['naturalness','自然さ'],['comfort','快適さ']].map(([key,name])=>`<label><input type="checkbox" data-set-secondary="${key}" ${p.secondary.includes(key)?'checked':''}> ${name}</label>`).join('')}</div>
 <details><summary>因子表を確認・編集（JSON）</summary><p class="aw-sub">開始差は「空気刺激 − 音」。正の値は空気刺激が後。強度補正は基準から−18〜0 dBです。</p><textarea id="set-factors" aria-label="実験条件の因子表" spellcheck="false">${esc(JSON.stringify(p.conditions,null,2))}</textarea></details>
 <div class="aw-row"><button id="set-generate" class="aw-primary">条件と試行順を生成</button><button id="set-open">manifestを開く</button><button id="set-save" ${experimentManifest?'':'disabled'}>manifestを保存</button><button id="set-export" ${experimentManifest?'':'disabled'}>Experiment Bundleを書き出す</button></div><p id="set-summary" role="status"></p><details><summary>生成した条件・試行順</summary><pre id="set-preview"></pre></details>
 </section><section class="ex-card"><h2>開始前の確認</h2><p>参加者ごとにイヤホンの左右、空気刺激の左右、快適なレベルを確認します。Unityで本試行を始める際にも確認を記録します。</p><div class="aw-row">${[['audio','イヤホン'],['air','空気刺激']].map(([kind,name])=>`<span>${name}</span><button data-calibration="${kind}" data-side="left">左を確認</button><button data-calibration="${kind}" data-side="right">右を確認</button>`).join('')}<button id="calibration-stop">停止</button><button id="calibration-settings">出力設定</button><button id="calibration-levels">基準レベルを調整</button></div><p class="aw-sub">現在の基準レベルで1回再生します。快適レベルを変更したら実験セットを再生成してください。Unityへ移す場合は実際に使う出力でも再確認します。</p></section>`;
}
function readExperimentPlan(){
 for(const id of ['set-repetitions','set-blocks','set-seed'])if(!$('#'+id).checkValidity()||$('#'+id).value==='')throw Error('反復・ブロック・シードの範囲を確認してください');
 return {template:$('#set-template').value,repetitions:+$('#set-repetitions').value,blocks:+$('#set-blocks').value,seed:+$('#set-seed').value,participants:$('#set-participants').value.split(',').map(s=>s.trim()),primary:$('#set-primary').value,secondary:$$('[data-set-secondary]:checked').map(el=>el.dataset.setSecondary),conditions:JSON.parse($('#set-factors').value)};
}
function showExperimentManifest(){
 if(!$('#set-summary'))return;
 const m=experimentManifest;
 $('#set-save').disabled=!m;$('#set-export').disabled=!m;
 $('#set-summary').textContent=m?`${m.plan.conditions.length} 条件 × ${m.plan.repetitions} 反復 × ${m.plan.blocks} ブロック · 参加者ごと ${m.trials.length/m.plan.participants.length} 試行 · project ${m.projectHash.slice(0,12)}`:'条件を生成すると、設定と参加者ごとの試行順を固定します。';
 $('#set-preview').textContent=m?JSON.stringify({conditions:m.plan.conditions,trials:m.trials.slice(0,200),note:'画面は先頭200試行まで。全試行はmanifestに保存します。'},null,2):'';
}
async function bindExperimentSet(){
 if(!experimentPlan.conditions.length){experimentPlan.conditions=await invoke('experiment_template',{template:experimentPlan.template});if(!$('#set-factors'))return;$('#set-factors').value=JSON.stringify(experimentPlan.conditions,null,2);}
 $('#set-template').onchange=()=>attempt(async()=>{experimentGeneration++;experimentPlan=readExperimentPlan();experimentPlan.conditions=await invoke('experiment_template',{template:experimentPlan.template});experimentManifest=null;experimentEditor()});
 for(const el of $$('.ex-plan input,.ex-plan select,.ex-plan textarea'))if(el.id!=='set-template')el.oninput=()=>{experimentGeneration++;experimentManifest=null;showExperimentManifest();try{experimentPlan=readExperimentPlan()}catch{}};
 $('#set-generate').onclick=()=>attempt(async()=>{experimentPlan=readExperimentPlan();const plan=clone(experimentPlan),generation=++experimentGeneration;await pending;const m=await invoke('generate_experiment_set',{plan});if(generation!==experimentGeneration)return;experimentManifest=m;showExperimentManifest();message('実験セットを生成しました');});
 $('#set-open').onclick=()=>attempt(async()=>{experimentGeneration++;const m=await invoke('experiment_manifest_file',{manifest:null});if(m){experimentManifest=m;experimentPlan=clone(m.plan);experimentEditor();message('manifestを読み込みました。元のプロジェクトは変更していません');}});
 $('#set-save').onclick=()=>attempt(async()=>{if(experimentManifest&&await invoke('experiment_manifest_file',{manifest:experimentManifest}))message('manifestを保存しました');});
 $('#set-export').onclick=()=>attempt(async()=>{if(!experimentManifest)return;const b=$('#set-export');b.disabled=true;message('条件ごとの音響を書き出します');try{const path=await invoke('export_experiment_set',{manifest:experimentManifest});if(path)message('書き出しました: '+path);else message('書き出しをキャンセルしました');}finally{b.disabled=!experimentManifest}});
 $$('[data-calibration]').forEach(b=>b.onclick=()=>attempt(async()=>{await invoke('preview_calibration',{modality:b.dataset.calibration,side:b.dataset.side});message('左右と快適レベルを確認してください');}));
 $('#calibration-stop').onclick=()=>attempt(()=>invoke('stop_audio'));
 $('#calibration-settings').onclick=()=>attempt(()=>goView('settings'));
 $('#calibration-levels').onclick=()=>attempt(()=>goView('timeline'));
 showExperimentManifest();
}
