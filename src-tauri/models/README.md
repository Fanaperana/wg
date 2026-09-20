# Speech-to-text models

These files are **not** committed to the repo because of their size. Download
them before building a release (they get bundled into the app via
`tauri.conf.json` > `bundle.resources`):

```powershell
pwsh -File scripts/download-models.ps1
```

Expected layout after download:

```
models/
  silero_vad.onnx
  sense-voice/
    model.int8.onnx
    tokens.txt
```

At runtime the app loads them from the bundled `Resource` directory (or this
folder during `tauri dev`).
