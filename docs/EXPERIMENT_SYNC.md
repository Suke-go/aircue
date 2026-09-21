# 実験同期 — v0.5.0

## 役割

Unityは試行順、画面提示、参加者の回答を管理する。AirCueは現在のタイムラインを事前レンダリングし、選択した音声機器で再生する。実験中に立体音響の編集値を遠隔変更せず、検証済みのタイムラインを`current`という刺激として扱う。

編集と実験操作を混在させないため、アプリには独立した「実験連携」画面を置く。この画面は次の順序だけを表示する。

1. タイムラインを確認する。
2. ローカル連携を開始する。
3. 現在のタイムラインと再生対象を準備する。
4. Unityから試行するか、画面の試行テストを使う。

準備後に波形、音声、タイムライン、出力設定を変更すると、準備済みデータを破棄して再準備を要求する。実験中の編集が、次の試行へ暗黙に混ざらないための動作である。

## 接続と権限

AirCueは`127.0.0.1`だけでTCP待受を行う。初期ポートは39100で、1024〜65535へ変更できる。LANからは接続できない。開始ごとにOSの乱数源から128 bitのセッショントークンを生成し、すべての要求で一致を確認する。トークンはアプリの実験連携画面からUnityの`AirCueExperimentClient`へ転記する。停止・再開すると新しいトークンになる。

通信はUTF-8の1行1 JSON。1行の上限は16 KiB、接続の読み取り待ちは30秒。クライアントは要求ごとに接続しても、同じ接続で複数要求を送ってもよい。識別子は1〜120文字の英数字と`-_.:`に限定する。

## プロトコル

全要求に`command`、`requestId`、`token`を含める。応答は同じ`requestId`と`ok`を返す。現在のプロトコルで使用できる刺激IDは`current`だけである。

```json
{"command":"status","requestId":"r1","token":"SESSION_TOKEN"}
{"command":"prepare","requestId":"r2","token":"SESSION_TOKEN","cueId":"current","target":"all"}
{"command":"play","requestId":"r3","token":"SESSION_TOKEN","trialId":"participant-01:trial-001","delayMs":100}
{"command":"stop","requestId":"r4","token":"SESSION_TOKEN","trialId":"participant-01:trial-001"}
```

`target`は`all`、`audio`、`air`。`delayMs`は50〜5000 msで、既定値は100 ms。`prepare`はプロジェクトを検証してレンダリングし、機器ID、ルーティング、再生対象とプロジェクトSHA-256を固定する。`prepare`と`play`の応答には`deviceId`と人が確認できる`output`も含む。プロジェクトの絶対パスや参加者情報は送信しない。

`play`は先頭へ4ch共通の無音フレームを追加して音声ストリームを開始する。応答の`scheduledUnixMs`は、再生位置と残りの無音時間から求めた予定刺激開始時刻である。新しい`play`は既存再生を置き換える。`stop`は短いフェードで停止する。

## 時刻と精度

記録するイベントは`serverStarted`、`serverStopped`、`prepared`、`invalidated`、`scheduled`、`started`、`finished`、`stopped`。イベントには連番、Unix時刻ms、試行ID、刺激ID、対象、予定時刻、再生位置、補足を持たせる。

`started`はAirCueの音声コールバックが予約した無音区間を通過したことを2 ms間隔で観測した時刻である。これはスピーカーや空気砲の物理出力をセンサーで観測した時刻ではない。OS、ドライバー、アンプ、スピーカー、約30 cmの伝搬による遅延を含まないため、1 ms級の物理同期を示す値としては扱わない。

視覚刺激とのソフトウェア同期では、Unityが先に`prepare`の成功を確認し、各試行で100 ms以上の`delayMs`を付けて`play`する。物理的な時間差が実験変数になる場合は、マイク・圧力センサー・光センサー・DAQで実測し、TTLなどのハードウェアトリガーを追加する必要がある。v0.5.0はTTL入出力を含まない。

## ログ

画面には直近200イベントのうち12件を表示する。全イベントは実行ファイルと同じ場所の`data/experiment-events.jsonl`へ追記する。アプリ再起動後もファイルを保持するが、画面上の履歴は現在の起動分だけである。

Unityの回答データには、最低限`trialId`、AirCueの`projectHash`、`scheduledUnixMs`、Unity側の条件と回答時刻を保存する。参加者を直接識別する情報はAirCueの`trialId`へ含めず、実験側の匿名化IDを使う。

## Unity

Unity書き出しに`AirCueExperimentClient.cs`を同梱する。コンポーネントへポートとセッショントークンを設定し、`CheckStatus`、`Prepare`、`Play`、`StopRemote`をUnityEventまたは実験スクリプトから呼ぶ。通信はワーカースレッドで行い、応答とエラーのUnityEventはメインスレッドで発火する。

完成音響をUnity自身で再生する`AirCuePlayer`と、デスクトップAirCueへ再生を依頼する`AirCueExperimentClient`は用途が異なる。実験でAirCueのASIO・4ch設定を使う場合はExperimentClientを使い、同じ刺激をAirCuePlayerから同時再生しない。

## 検証範囲

Rustテストはプロトコルの複数要求・不正JSON・厳格なフィールド・識別子・4ch無音予約を確認する。UIは停止時に準備・テストを無効化し、開始後に接続情報を表示し、編集後に準備を無効化する。Unity 6の検証プロジェクトでクライアントC#のコンパイルを確認する。

実機のASIO/4ch再生、UnityとAirCueの長時間クロック差、物理刺激の開始時刻、複数PC同期は別途実測する。
