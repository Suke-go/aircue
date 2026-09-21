# 実行環境に依存しない実験データ

このフォルダーの標準書き出しにはUnity・C#・専用ランタイムは不要です。Python、PsychoPy、自作アプリなどでWAVとJSONを読み込めます。各環境向けの実験画面・回答取得コードは含みません。

- `Experiment.aircueexp`: UTF-8 JSON。`plan.conditions` が因子表、`trials` が固定済みの参加者別試行順。`stimuli[].sequence` が対応するメタデータへの相対パスです。
- `Sequences/Sequence-<conditionId>/Sequence.aircueseq`: UTF-8 JSON。独自拡張子ですが通常のJSONとして読めます。`sampleRate`、`frames`、`routing`、`markers[].startFrame` を含みます。
- 同じフォルダーの `routed-4ch.wav`: 48 kHz、24-bit PCM、4ch。`routing` はイヤホンL/R・空気刺激L/Rの順に記した1始まりのWAVチャンネル番号です。4chを一つのストリームで再生するとファイル内の開始差が保たれます。
- `headphones.wav`: 同じ長さの2ch完成音響。HRTFは焼き込み済みです。追加の空間化は不要です。
- `air-left.wav` / `air-right.wav`: 同じ長さの1ch空気刺激。個別ファイルを別々に再生する場合、実行環境側で同期を確保してください。
- `Sequences/Sequence-calibration-0..3`: イヤホン左／右、空気刺激左／右の確認用刺激。試行順には含みません。
- `checksums.json`: 各本試行刺激の4ch WAVに対するSHA-256。

各参加者について `trials` を `participantId` で絞り込み、格納順に提示します。条件IDを使い対応する刺激を選びます。刺激の先頭には余白があり、音開始は200 ms、空気刺激開始は200＋`onsetDiffMs` msです。条件に存在しない刺激は再生されません。反応時間の基準は、存在する刺激の最も早い予定開始としてください。

開始前の左右・快適レベル確認、休憩、回答、中止、ログ保存は実行側の責務です。最低限、セッションID、参加者ID、試行ID、条件ID、projectHash、刺激revision、実際の出力構成、予定開始・ソフトウェアで観測した開始・回答の時刻と時計種別、回答、評定、中止理由を保存してください。ソフトウェア時刻は物理刺激開始の実測値ではありません。

Unityを使用する場合だけ「Unity用Bundle」を選んでください。同じPCM/JSONにImporterとRunnerを追加します。標準Bundleの利用にUnityのインストールは不要です。
