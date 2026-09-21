# AirCue → Unity

1. 書き出した `AirCueUnity` フォルダーをUnityプロジェクトの `Assets` へコピーします。追加の書き出しでは同じ `Assets/AirCueUnity` へ統合してください。スクリプトを別のフォルダーへ重複配置しないでください。
2. コンパイルと音声取り込みが完了すると、`Sequences/Sequence-…/Sequence.aircueseq` が再生用Prefabになります。
3. そのPrefabをシーンへドラッグし、Audio ListenerのあるシーンでPlayします。初期設定ではイヤホン用ステレオだけを再生します。

`AirCuePlayer` の `Play On Start` を外せば、UI ButtonのOnClickやスクリプトから `Play()` / `Stop()` を呼べます。同じPlayerのPlayは再スタートします。複数Playerの同時再生は音量が加算されます。

## AirCueデスクトップとの実験同期

AirCueのASIO・4ch出力を使ってUnityから試行を開始する場合は、空のGameObjectへ`AirCueExperimentClient`を追加します。AirCueの「実験連携」で接続を開始し、表示されたポートとセッショントークンをInspectorへ入力します。

`CheckStatus`、`Prepare`、`Play`、`StopRemote`をUnityEventまたは実験スクリプトから呼べます。`responseReceived`にはAirCueのJSON応答、`errorReceived`には接続エラーが渡ります。Prepareの成功を確認してから試行ごとに一意の`trialId`を設定し、Playを呼びます。通信中もUnityのメインスレッドを停止しません。

デスクトップ同期では`AirCuePlayer`を同時に再生しないでください。AirCuePlayerは書き出した音声をUnity自身で鳴らす用途、ExperimentClientはデスクトップAirCueへ再生を依頼する用途です。詳細は `docs/EXPERIMENT_SYNC.md` を参照してください。

## 音響と出力

音源の移動・高さ・距離・レベルはWAVへ反映済みです。立体音響オフのクリップはステレオのままです。Unityで追加のSpatializerや3D減衰を設定する必要はありません。頭部追跡やワールド座標への追従は行いません。UnityのAudio Listener音量など、プロジェクト全体の設定は再生結果に影響します。

- `headphones.wav`: イヤホン左右。元の物理ch割り当てに関係なく、左右順の2chです。
- `routed-4ch.wav`: AirCueの出力設定どおりの物理1〜4ch。音声と空気砲が同じWAVに入っています。
- `air-left.wav` / `air-right.wav`: 左右それぞれの空気砲。先頭の無音を含め、他のWAVと同じ長さです。
- `Sequence.aircueseq`: サンプルレート、フレーム数、ルーティング、クリップの開始・長さ、音源経路の情報。

空気砲をUnityから同時再生する場合は、Unity AudioのSpeaker ModeをQuadにし、実機で4つの出力端子との対応を確認した上でPlayerのOutputをRouted Quadへ変更します。Quad以外では再生を拒否し、ステレオへの代替再生をしません。Unityのオーディオデバイスとドライバーを使用するため、AirCueのASIO機器選択は引き継ぎません。OSやドライバーによるダウンミックスはアプリ側では検出できません。Rubix44実機の4ch出力は別途確認が必要です。

Windows / Unity 6を対象とします。WAVは48 kHz・24 bit PCM。付属Importerが音声をPCM・Decompress On Load・元のサンプルレート・ステレオ維持に設定します。プラットフォーム固有のAudioImporter上書き設定は追加しないでください。

## タイミング

音声と空気砲の相対時刻は単一の4ch WAV内でサンプル単位に固定されています。Playerは `AudioSource.PlayScheduled` を使用します。`ScheduledDspTime` が再生開始のDSP時刻です。外部連携には `Scheduled` イベントと `sequence.manifest.markers` を参照できます。マーカーの開始時刻は `ScheduledDspTime + startFrame / 48000.0` です。UnityのUpdateによるイベント通知は空気砲の正確な駆動用途には使わないでください。

書き出しごとに新しいSequenceフォルダーができます。既存シーンの参照は勝手に置換しません。新しいPrefabへ置き換えるか、PlayerのSequence参照を新しい書き出しのサブアセットへ変更します。

出典・利用条件は同梱の `LICENSE` と `THIRD_PARTY_HRTF.txt` を参照してください。


## v0.6 Experiment Bundle

`Experiment.aircueexp` をシーンへドラッグすると試行Runnerを生成します。自動再生はしません。匿名参加者を `BeginParticipant` で選び、`PlayCalibration` と `ConfirmCalibration` で左右・快適さを確認してから `PlayNext`、`SubmitResponse`、`Abort` を呼びます。空気刺激にはRoutedQuadが必要です。回答画面・休憩は研究スクリプトで実装してください。詳細は同梱 `docs/EXPERIMENT_DESIGN.md` を参照してください。
