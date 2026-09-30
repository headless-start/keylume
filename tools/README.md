# Dev tools

Small scripts used while developing. None of them ship with the app.

| Script | What |
|---|---|
| `kcdp.py` | evaluate JS in / screenshot the running Keylume window over DevTools (port 9334) |
| `kshot_size.py` | screenshot the window at an emulated size (layout checks at 1100/1600/2560 px) |
| `hover.py` | hover a Library card by name and capture its preview animation |
| `make_icon.mjs` | render `src-tauri/app-icon.png` from `src/assets/logo.svg` (headless Chromium), then `npx tauri icon src-tauri/app-icon.png` |
| `fmt_themes.py` | format theme files in the house style |
| `flag_grid.py` | print flags (or any per-key profile) as letter grids, from the generated library |
| `readme_shots.mjs` | take the README screenshots and clips (`docs/images/`) from the browser preview |
| `ui_shots.mjs` | screenshot the browser preview in a few steps (`click:Customise nav:Create shot:x`), for checking a UI change; see `docs/DESIGN.md`. `eval:window.__keylumeMock.setConnected(false)` (or `setPaused`, `failNext`) shows the other states; `media:prefers-reduced-motion=reduce` looks like Windows with animation effects off |
| `third_party_notices.mjs` | write `src-tauri/THIRD-PARTY-NOTICES.txt`, the licences of everything the app ships (run by `tauri build`) |
| `sign.ps1` | sign a file with the release certificate (the release workflow calls it; see `docs/RELEASING.md`) |

To open the DevTools port, launch the installed app with
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9334`, and relaunch
it normally afterwards.

`readme_shots.mjs` starts its own Vite server and a headless Chromium (Playwright's, or
`CHROME=/path/to/chrome`), and needs Python with Pillow for the animated WebP. On Node 20
run it as `node --experimental-websocket tools/readme_shots.mjs`; `ONLY=library,hover`
retakes just those scenes.
