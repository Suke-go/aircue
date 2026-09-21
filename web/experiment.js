'use strict';
let experimentLastPoll=0,experimentState=null;
const experimentEventNames={serverStarted:'連携開始',serverStopped:'連携停止',prepared:'準備完了',invalidated:'再準備が必要',scheduled:'再生予約',started:'刺激開始',finished:'完了',stopped:'停止'};
function experimentEditor(){
 const air=project.clips.length,audio=project.audioClips.length;
 $('#app').innerHTML=`<section class="ex-page"><header class="ex-intro"><div><h1>実験</h1><p>条件と試行順をまとめ、AirCue内で実行するか、WAV・JSONを任意の実験環境へ渡します。</p></div></header>
 ${experimentSetMarkup()}${studyMarkup()}<details><summary>ローカル連携 · 現在のタイムラインを外部アプリから再生 <span id="ex-server-badge" class="ex-badge">確認中</span></summary><div class="ex-grid"><section class="ex-main"><article class="ex-card"><div class="ex-step"><span>1</span><div><strong>タイムライン</strong><p>${project.durationMs.toFixed(0)} ms · オーディオ ${audio} · 空気砲 ${air}</p></div></div><button id="ex-open-timeline">タイムラインを編集</button></article>
 <article class="ex-card"><div class="ex-step"><span>2</span><div><strong>ローカル連携</strong><p>このPCの実験アプリを、セッショントークンで接続します。</p></div></div><div class="ex-fields"><label>ポート<input id="ex-port" type="number" min="1024" max="65535" value="39100"></label><button id="ex-server" class="aw-primary">連携を開始</button></div></article>
 <article class="ex-card"><div class="ex-step"><span>3</span><div><strong>刺激を準備</strong><p>編集後は再度準備します。準備した内容は、その後の編集から切り離されます。</p></div></div><div class="ex-fields"><label>再生対象<select id="ex-target"><option value="all">すべて</option><option value="audio">オーディオのみ</option><option value="air">空気砲のみ</option></select></label><button id="ex-prepare">現在のタイムラインを準備</button></div><div id="ex-prepared" class="aw-sub"></div></article>
 <article class="ex-card"><div class="ex-step"><span>4</span><div><strong>試行テスト</strong><p>外部アプリと同じ手順で、試行IDを記録して予約再生します。</p></div></div><div class="ex-fields ex-test"><label>試行ID<input id="ex-trial" value="test-001" maxlength="120" pattern="[A-Za-z0-9_.:-]+"></label><label>開始待ち<input id="ex-delay" type="number" min="50" max="5000" value="100"> ms</label><button id="ex-test" class="aw-primary">テスト再生</button><button id="ex-stop">停止</button></div></article></section>
 <aside class="ex-side"><section class="ex-card"><strong>外部アプリ接続情報</strong><div id="ex-connection" class="ex-connection aw-sub">連携を開始すると表示します。</div><details><summary>通信仕様</summary><p>1行1 JSON。順番は status → prepare → play → stop です。Unity用クライアントは書き出しに同梱します。</p></details></section><section class="ex-card"><div class="aw-row aw-between"><strong>最近の記録</strong><span id="ex-log-path" class="aw-sub"></span></div><div id="ex-events" class="ex-events"><p class="aw-sub">記録はまだありません。</p></div></section></aside></div></details></section>`;
 $('#ex-open-timeline').onclick=()=>attempt(()=>goView('timeline'));
 $('#ex-server').onclick=()=>attempt(toggleExperimentServer);
 $('#ex-prepare').onclick=()=>attempt(async()=>{const result=await invoke('prepare_experiment',{target:$('#ex-target').value});message(`準備しました: ${result.durationMs.toFixed(0)} ms`);await refreshExperiment(true)});
 $('#ex-test').onclick=()=>attempt(async()=>{const trialId=$('#ex-trial').value,delayMs=+$('#ex-delay').value;if(!$('#ex-trial').checkValidity()||!$('#ex-delay').checkValidity())throw Error('試行IDと開始待ちを確認してください');const result=await invoke('play_experiment_test',{trialId,delayMs});message(`試行 ${trialId} を予約しました: ${new Date(result.scheduledUnixMs).toLocaleTimeString('ja-JP',{fractionalSecondDigits:3})}`);await refreshExperiment(true)});
 $('#ex-stop').onclick=()=>attempt(async()=>{await invoke('stop_experiment_trial');message('試行を停止しました');await refreshExperiment(true)});
 attempt(bindExperimentSet);
 bindStudy();
 refreshExperiment(true);
}
async function toggleExperimentServer(){
 const current=await invoke('experiment_status');
 if(current.running){await invoke('stop_experiment_server');message('実験連携を停止しました')}
 else{const port=+$('#ex-port').value;if(!$('#ex-port').checkValidity())throw Error('ポートは1024〜65535です');await invoke('start_experiment_server',{port});message('実験連携を開始しました')}
 await refreshExperiment(true);
}
async function refreshExperiment(force=false){
 if(view!=='experiment'||!$('#ex-server-badge'))return;
 const now=Date.now();if(!force&&now-experimentLastPoll<500)return;experimentLastPoll=now;
 const s=await invoke('experiment_status');if(view!=='experiment'||!$('#ex-server-badge'))return;experimentState=s;
 await refreshStudy();
 if(view!=='experiment'||!$('#ex-server-badge'))return;
 $('#ex-server-badge').textContent=s.running?'待機中':'停止中';$('#ex-server-badge').classList.toggle('ex-live',s.running);
 $('#ex-server').textContent=s.running?'連携を停止':'連携を開始';$('#ex-port').disabled=s.running;$('#ex-port').value=s.port;
 $('#ex-prepare').disabled=!s.running;$('#ex-test').disabled=!s.running||!s.prepared;$('#ex-stop').disabled=!s.activeTrialId;
 $('#ex-prepared').textContent=s.prepared?`準備済み · ${s.target==='all'?'すべて':s.target==='audio'?'オーディオ':'空気砲'} · ${s.durationMs.toFixed(0)} ms · ID ${s.projectHash.slice(0,8)}`:'未準備';
 $('#ex-connection').innerHTML=s.running?`<dl><dt>アドレス</dt><dd>${s.host}:${s.port}</dd><dt>トークン</dt><dd><code>${s.sessionToken}</code></dd><dt>状態</dt><dd>${s.activeTrialId?'試行 '+esc(s.activeTrialId)+' を再生中':s.prepared?'刺激を準備済み':'接続待ち'}</dd>${s.prepared?`<dt>出力</dt><dd>${esc(s.output)}</dd>`:''}</dl>`:'連携を開始すると表示します。';
 $('#ex-log-path').title=s.logPath;$('#ex-log-path').textContent='JSONLへ保存';
 $('#ex-events').innerHTML=s.events.length?s.events.slice(0,12).map(e=>`<div class="ex-event"><time>${new Date(e.unixMs).toLocaleTimeString('ja-JP',{fractionalSecondDigits:3})}</time><strong>${experimentEventNames[e.event]||esc(e.event)}</strong><span>${e.trialId?esc(e.trialId):''}</span></div>`).join(''):'<p class="aw-sub">記録はまだありません。</p>';
}
