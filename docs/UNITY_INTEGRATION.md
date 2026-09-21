# Unity連携 — v0.4.0

## 採用方式

AirCueで完成させた音響をそのまま再生する。HRTF、経路、距離減衰、クリップ補正、配置時刻を48 kHz・24 bit PCMのWAVへ反映し、Unityでは2DのAudioSourceで再生する。VRの頭の動きやUnityオブジェクトの移動に合わせた再定位は対象外。

音源位置のメタデータも残すが、UnityのPlayerはそれを再レンダリングしない。これによりAirCueとUnityでSpatializerや距離モデルの差が生じることを避ける。

## 書き出し

タイムラインの「Unityへ書き出し」から保存先の親フォルダーを選ぶ。プロジェクトと音声ピークを検証し、新しい `AirCue-<ID>` フォルダーを作成する。その中の `AirCueUnity` をUnityのAssetsへ統合する。毎回異なるSequenceフォルダーを作り、既存アセットの上書きやシーンの参照変更はしない。書き出しに失敗した場合は、今回作成した不完全なフォルダーだけを除去する。

```text
AirCueUnity/
  Runtime/                 再生コンポーネントとSequenceデータ
  Editor/                  自動取り込み
  Sequences/Sequence-ID/
    Sequence.aircueseq     バージョン付きJSON
    headphones.wav         2ch・イヤホン左／右
    routed-4ch.wav         4ch・AirCueの物理ch割り当て
    air-left.wav           1ch・左空気砲
    air-right.wav          1ch・右空気砲
  README.md
  LICENSE
  THIRD_PARTY_HRTF.txt
```

全WAVは同じ開始・終了フレームを持つ。イヤホン版と空気砲の単独版は、同じ4chレンダリング結果からチャンネルを抽出する。配置・ゲイン・HRTFを再計算し直すことによる差を作らない。

JSONのschemaVersionは1。sampleRate、frames、routing（イヤホン左右・空気砲左右の順で1始まり）、markers（id、name、lane、startFrame、durationFrames）、audioClips（音源・経路情報）を含む。端末のデバイスIDや元音声の絶対パスは含めない。クリップの開始・長さは実際のWAVと同じフレームへの丸めを使う。

## Unityの取り込み・実行

ScriptedImporterが `.aircueseq` と隣接WAVの依存関係を登録し、再生用PrefabとAirCueSequenceサブアセットを生成する。AudioImporterは対象のイヤホン・4chファイルに限り、PCM、Decompress On Load、48 kHz維持、モノラル化なし、事前読み込みを設定する。メタデータのバージョンとルーティング、音声のチャンネル数・周波数・フレーム数が不一致なら取り込みエラーとする。

AirCuePlayerの初期出力はHeadphones。Play On Startを無効にして `Play()` / `Stop()` をゲーム側から呼べる。Playは同じPlayerの既存再生を停止して開始し直す。AudioSourceのSpatial Blend、パン、ピッチ、追加エフェクトとMixer参照を初期化し、定位済みWAVに追加の空間処理をかけない。シーンにはAudio Listenerが必要。

4ch版のRouted QuadはUnityのSpeaker ModeがQuadの場合のみ開始する。2chへのフォールバックは行わない。デバイス選択・ドライバー・物理端子の対応はUnityとOSの設定に依存し、AirCueのCPAL/ASIO設定は移植しない。OS側のダウンミックスや実端子対応はソフトウェア検査だけでは保証できない。

AudioSource.PlayScheduledでDSP時刻を指定する。各マーカーの予定時刻は `ScheduledDspTime + startFrame / sampleRate`。1本の4chファイル内では音声と空気砲のタイミングを共有する。別デバイス同士のクロック同期、物理音響遅延、メインスレッドのUpdateによるサンプル精度の空気砲制御は対象外。

## 検証

Rustテストで入れ替えたch割り当てを使用し、2ch・4ch・空気砲のサンプル一致、無音区間、フレーム数、マーカーを確認する。空のタイムラインや不正なプロジェクトは書き出し前に拒否する。

Unity 6000.4.3f1の専用プロジェクトで、`tests/unity-project.json` の書き出しを取り込み、Prefab、サブアセット、PCM値、チャンネル、時刻、再取り込み後の参照を検証した。検証コードは `unity/Tests/Editor/AirCueImportVerification.cs`。このテストコードは配布フォルダーに含めない。GitHub ActionsではUnityライセンスを設定していないため、Rust・JavaScript・Tauriの検証を実行し、Unity検証はローカルで行う。

CLIでも `AirCue.exe --export-unity project.aircue destination` で書き出せる。`.aircue` の音声素材は同名の `.media` から読み込む。検証用JSONの場合は同じフォルダーを素材の基準とする。失敗時は終了コード1を返す。

再検証は新規Unityプロジェクトへ書き出しと検証C#をコピーし、Unity Editorに `-batchmode -nographics -projectPath <専用プロジェクト> -executeMethod AirCueImportVerification.Run -quit -logFile <ログ>` を指定する。成功ログは `AIRCUE_UNITY_VERIFIED`。実音声、Rubix44、空気砲による再生確認は含まない。

## 参照API

- [Unity ScriptedImporter](https://docs.unity3d.com/jp/current/ScriptReference/AssetImporters.ScriptedImporter.html)
- [AudioImporterSampleSettings](https://docs.unity3d.com/cn/6000.0/ScriptReference/AudioImporterSampleSettings.html)
- [AudioSource.PlayScheduled](https://docs.unity.com/en-us/engine/6000.3/script-reference/unityengine/audiosource/playscheduled)
