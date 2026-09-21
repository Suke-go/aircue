$ErrorActionPreference = 'Stop'
$buildToolsDir = Join-Path $PSScriptRoot '../.build-tools'
New-Item -ItemType Directory -Force -Path $buildToolsDir | Out-Null
$sdkZip = Join-Path $buildToolsDir 'asio.zip'
$sdkUrl = 'https://download.steinberg.net/sdk_downloads/ASIO-SDK_2.3.4_2025-10-15.zip'
if (-not (Test-Path -LiteralPath $sdkZip)) { Invoke-WebRequest -Uri $sdkUrl -OutFile $sdkZip }
$expected = 'D5EBF0C20DD2C5F43771FD0C1418F4B361BF52434EE670097CFA6B3A335E2ECA'
if ((Get-FileHash -LiteralPath $sdkZip -Algorithm SHA256).Hash -ne $expected) { throw 'ASIO SDK checksum mismatch' }
$sdkExtract = Join-Path $buildToolsDir 'asio'
if (-not (Test-Path -LiteralPath $sdkExtract)) { Expand-Archive -LiteralPath $sdkZip -DestinationPath $sdkExtract }
$header = Get-ChildItem -LiteralPath $sdkExtract -Filter 'asio.h' -Recurse | Select-Object -First 1
if (-not $header) { throw 'ASIO SDK header not found' }
$env:CPAL_ASIO_DIR = $header.Directory.Parent.FullName
$pythonLib = Join-Path $buildToolsDir 'python'
python -m pip install --disable-pip-version-check --target $pythonLib libclang==18.1.1
if ($LASTEXITCODE -ne 0) { throw 'libclang installation failed' }
$env:LIBCLANG_PATH = Join-Path $pythonLib 'clang/native'
if (-not (Test-Path -LiteralPath (Join-Path $env:LIBCLANG_PATH 'libclang.dll'))) { throw 'libclang.dll not found' }
if ($env:GITHUB_ENV) {
  "CPAL_ASIO_DIR=$env:CPAL_ASIO_DIR" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
  "LIBCLANG_PATH=$env:LIBCLANG_PATH" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
}
Write-Host 'ASIO SDK and libclang are ready.'
