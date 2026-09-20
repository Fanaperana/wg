# Downloads the local speech-to-text models bundled with the app.
#
#   models/
#     silero_vad.onnx
#     sense-voice/
#       model.int8.onnx
#       tokens.txt
#
# Run from the repo root (or anywhere):
#   pwsh -File scripts/download-models.ps1

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$modelsDir = Join-Path $root 'src-tauri/models'
$senseDir = Join-Path $modelsDir 'sense-voice'
New-Item -ItemType Directory -Force -Path $modelsDir | Out-Null

$base = 'https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models'

# --- Silero VAD ---
$vadPath = Join-Path $modelsDir 'silero_vad.onnx'
if (Test-Path $vadPath) {
    Write-Host "VAD already present: $vadPath"
} else {
    Write-Host 'Downloading Silero VAD...'
    Invoke-WebRequest -Uri "$base/silero_vad.onnx" -OutFile $vadPath
}

# --- SenseVoice offline recognizer ---
if ((Test-Path (Join-Path $senseDir 'model.int8.onnx')) -and
    (Test-Path (Join-Path $senseDir 'tokens.txt'))) {
    Write-Host "SenseVoice already present: $senseDir"
} else {
    $archiveName = 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2'
    $archivePath = Join-Path $modelsDir $archiveName
    Write-Host 'Downloading SenseVoice model (~1 GB)...'
    Invoke-WebRequest -Uri "$base/$archiveName" -OutFile $archivePath

    Write-Host 'Extracting...'
    tar -xjf $archivePath -C $modelsDir
    Remove-Item $archivePath -Force

    $extracted = Join-Path $modelsDir 'sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17'
    New-Item -ItemType Directory -Force -Path $senseDir | Out-Null
    Copy-Item (Join-Path $extracted 'model.int8.onnx') (Join-Path $senseDir 'model.int8.onnx') -Force
    Copy-Item (Join-Path $extracted 'tokens.txt') (Join-Path $senseDir 'tokens.txt') -Force
    Remove-Item $extracted -Recurse -Force
}

Write-Host 'Done. Models are in src-tauri/models.' -ForegroundColor Green
