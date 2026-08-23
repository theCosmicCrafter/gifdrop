# Contributing to GifDrop

Thanks for your interest in contributing! This is a small project, but contributions are welcome.

## Ways to contribute

- **Bug reports** — open an issue with the bug template
- **Feature requests** — open an issue with the feature template
- **Pull requests** — fix a bug or implement a feature
- **Documentation** — improve the README or add examples
- **Examples** — add sample input/output pairs to `examples/`

## Development setup

### Prerequisites

- Node.js 18+
- Rust (stable)
- ffmpeg + ffprobe on PATH
- Windows 10/11 (currently Windows-only)

### Getting started

```powershell
git clone https://github.com/theCosmicCrafter/gifdrop.git
cd gifdrop
npm install
npm run tauri dev
```

This launches the app with hot-reload. Edit `src/main.ts` or `src/styles.css` and changes appear instantly.

### Running tests

```powershell
cd src-tauri
cargo test
```

### Building

```powershell
npm run tauri build
```

## Pull request process

1. Fork the repo and create a branch from `main`
2. Make your changes
3. Test your changes:
   - `cd src-tauri && cargo test` (Rust tests)
   - `npm run build` (TypeScript compiles cleanly)
   - Manually test the app with `npm run tauri dev`
4. Update the README if you added a new feature
5. Open a pull request with a clear description

### Code style

- **Rust**: Follow standard `cargo fmt` formatting. Run `cargo clippy` and fix warnings.
- **TypeScript**: The project uses strict mode. No `any` types unless absolutely necessary (the drag-drop event payload is an exception).
- **CSS**: Keep styles in `src/styles.css`. Use CSS variables from `:root` for colors.

### Commit messages

Use clear, descriptive commit messages:

```
Add APNG transparency support
Fix ffmpeg not found on winget installs
Update README with troubleshooting section
```

## Architecture overview

- `src-tauri/src/lib.rs` — all backend logic (ffmpeg invocation, progress streaming, binary location)
- `src/main.ts` — all frontend logic (drag-drop, file picker, conversion, preview, feedback)
- `src/styles.css` — all styling
- `index.html` — app structure
- `src-tauri/tauri.conf.json` — Tauri config (window size, security, bundling)
- `src-tauri/capabilities/default.json` — permission scopes

## Areas for contribution

- **Cross-platform support** — currently Windows-only. macOS and Linux support would require testing the ffmpeg location logic and window flags.
- **More output formats** — WebP animation, WebM, etc.
- **Trim controls** — set start/end time for the output
- **Crop controls** — crop the video before converting
- **Batch conversion** — drop multiple files and convert them all
- **Custom output directory** — option to save somewhere other than the source folder
- **Settings persistence** — remember last-used format and FPS

## Questions?

Open an issue with the question label.
