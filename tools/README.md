# Bundled tools

Same layout as the C# studio. Binaries are gitignored.

```
tools/ffmpeg/ffmpeg.exe
tools/ffmpeg/ffprobe.exe
tools/mpv/mpv.exe
```

**Detect bundled tools** in Settings must resolve these paths relative to the app executable.

Do not require Scoop, Chocolatey, or MSYS for end users.
