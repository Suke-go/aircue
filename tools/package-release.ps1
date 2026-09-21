param([switch]$WithSource)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$version = (Get-Content -LiteralPath (Join-Path $repo 'tauri.conf.json') -Raw | ConvertFrom-Json).version
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
$dist = Join-Path $repo 'dist'
$portable = Join-Path $dist "AirCue-$version-windows-x64"
New-Item -ItemType Directory -Force -Path $portable | Out-Null
Copy-Item -LiteralPath (Join-Path $targetDir 'release/AirCue.exe') -Destination $portable
foreach ($file in @('README.md','LICENSE','NOTICE.txt','THIRD_PARTY_ASIO.txt','THIRD_PARTY_HRTF.txt')) {
  Copy-Item -LiteralPath (Join-Path $repo $file) -Destination $portable
}
Compress-Archive -Path (Join-Path $portable '*') -DestinationPath "$portable.zip" -Force
$installers = @(Get-ChildItem -LiteralPath (Join-Path $targetDir 'release/bundle/nsis') -Filter "*_${version}_*-setup.exe")
if ($installers.Count -ne 1) { throw 'Expected exactly one NSIS installer' }
Copy-Item -LiteralPath $installers[0].FullName -Destination (Join-Path $dist "AirCue-$version-windows-x64-setup.exe")
if ($WithSource) {
  $sourceDir = Join-Path $dist "AirCue-$version-source"
  New-Item -ItemType Directory -Force -Path $sourceDir | Out-Null
  $archive = Join-Path $dist 'tracked-source.zip'
  git -C $repo archive --format=zip --output=$archive HEAD
  if ($LASTEXITCODE -ne 0) { throw 'Source archive failed' }
  Expand-Archive -LiteralPath $archive -DestinationPath $sourceDir -Force
  Remove-Item -LiteralPath $archive
  $cargoConfig = Join-Path $sourceDir '.cargo'
  New-Item -ItemType Directory -Force -Path $cargoConfig | Out-Null
  Push-Location $sourceDir
  try {
    cargo vendor --locked vendor | Out-File -FilePath (Join-Path $cargoConfig 'config.toml') -Encoding utf8
    if ($LASTEXITCODE -ne 0) { throw 'Vendoring Rust dependencies failed' }
  } finally { Pop-Location }
  if (-not $env:CPAL_ASIO_DIR) { throw 'CPAL_ASIO_DIR is required for corresponding source' }
  Copy-Item -LiteralPath $env:CPAL_ASIO_DIR -Destination (Join-Path $sourceDir 'ASIOSDK') -Recurse
  tar -czf (Join-Path $dist "AirCue-$version-source.tar.gz") -C $dist "AirCue-$version-source"
  if ($LASTEXITCODE -ne 0) { throw 'Corresponding source packaging failed' }
}
$assets = Get-ChildItem -LiteralPath $dist -File | Where-Object { $_.Name -like "AirCue-$version-*" -and $_.Extension -in @('.zip','.exe','.gz') }
$assets | ForEach-Object { "{0}  {1}" -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $_.Name } |
  Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Encoding utf8
