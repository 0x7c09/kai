# kai

A minimal Tauri desktop pet that reuses hatch-pet packages from:

```text
${CODEX_HOME:-$HOME/.codex}/pets/<pet-id>/
  pet.json
  spritesheet.webp
```

The current MVP opens a transparent, borderless, always-on-top window and plays the first available hatch-pet package.

## Development

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The macOS app bundle is written to:

```text
src-tauri/target/release/bundle/macos/kai.app
```

## Controls

- Click: wave
- Double-click: jump
- Right-click: quit
- `Esc` or `q`: quit
- `1`-`5`: switch test states
