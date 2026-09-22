# macOS版のビルドと署名

macOS 11以降。Apple Silicon (`arm64`) とIntel (`x64`) をそれぞれのGitHub-hosted runnerでテストし、`.app`入りZIPとDMGを作ります。音声出力はCoreAudioで、ASIO SDKは不要です。

## GitHub Actions

Actions → **Desktop build and release** → Run workflow → platform: **macos**。
`main`へのpush、PR、`v*`タグではWindowsと両Mac版が動きます。手動実行は選択したブランチをビルドします。

```sh
gh workflow run windows.yml --ref YOUR_BRANCH -f platform=macos
```

完了したrunのArtifactsから `AirCue-macos-arm64` または `AirCue-macos-x64` を取得します。保存期間は14日です。タグ実行では既存のReleaseへも追加されます。各artifactにはDMG、app ZIP、Rust依存ソースを含む対応ソース、SHA256チェックサムを入れています。

現段階はアドホック署名（identity `-`）のみです。ビルドにAppleログイン・証明書・GitHub Secretsは不要です。Appleによる署名者の確認と公証は含まれず、ダウンロードしたアプリを開く際にはmacOSの許可操作が必要な場合があります。DMGからアプリケーションフォルダへコピーして使います。

作業データは `~/Library/Application Support/local.aircue.studio/` に保存します。`.app`の中には書き込みません。`AIRCUE_DATA_DIR`による明示的な保存先指定も使えます。初回は出力機器を選択してください。4ch同時再生は対応するCoreAudio機器が必要です。

## Macで配布用の認証をする場合

1. Apple Developer Programで **Developer ID Application** 証明書を作成し、秘密鍵とともにMacのキーチェーンへ入れます。
2. `security find-identity -v -p codesigning` で署名IDを確認し、`APPLE_SIGNING_IDENTITY`へ設定します。既定のアドホック署名をこの環境変数で置き換えます。
3. 公証には `APPLE_ID`、`APPLE_TEAM_ID`、`APPLE_PASSWORD`（通常のログインパスワードではなくアプリ用パスワード）を環境変数で設定します。App Store Connect API認証も利用できます。秘密情報をソース管理へ入れないでください。
4. Macで同じソースを再ビルドします。Tauriが署名・公証を行います。

```sh
npm ci
rustup toolchain install 1.95.0
export RUSTUP_TOOLCHAIN=1.95.0
rustup target add --toolchain 1.95.0 aarch64-apple-darwin
npm run tauri -- build --target aarch64-apple-darwin --bundles app,dmg -- --locked
```

Intel Macはターゲットを `x86_64-apple-darwin` に変えます。Xcode Command Line ToolsとNode.js 24が必要です。対応ソースの `.cargo/config.toml` と `vendor/` を利用する場合、Cargoの `--offline` も指定できます（npm依存は別途 `npm ci` が必要）。

後からCIにも署名を移せますが、現ワークフローには証明書や公証用認証情報を渡していません。

公式資料: [Tauri macOS署名・公証](https://v2.tauri.app/distribute/sign/macos/)、[GitHub macOS runner](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)。
