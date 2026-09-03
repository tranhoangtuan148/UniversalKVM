# AGENTS.md

Working notes for UniversalKVM. Read this before changing code.

## What this is

UniversalKVM is a software KVM: a desktop app that captures keyboard and mouse events on
one computer and replays them on another over the local network. It also syncs the
clipboard, transfers files, and lets the user define which screen edges the cursor may
cross.

Tauri 2 app. Rust backend in `src-tauri/`, React + TypeScript frontend in `src/`,
bundled by Vite. Package manager is **pnpm**.

## Commands

```sh
pnpm install                              # install node modules
pnpm build                                # typecheck (tsc) and build the frontend only
pnpm tauri dev                            # run the app in development
pnpm tauri build --debug --no-bundle      # fastest full compile
pnpm tauri build                          # release build with installers
cd src-tauri && cargo check               # check the Rust side alone
```

`pnpm build` runs `tsc` first, so it fails on type errors. Use it as the frontend gate.

Running `pnpm tauri dev` opens a real window and can capture real input devices. Do not
launch it without the user asking.

## Layout

```
src/
  App.tsx              global state, Tauri event listeners, GlobalContext
  AppTabs.tsx          top bar: brand plus tab navigation
  App.css              design tokens and shared primitives
  interfaces/global.ts every shape crossing the Rust boundary
  pages/               Home (machines and monitor layout), Devices, Settings, Logging, About
  components/          Monitors (frame and edit mode), MonitorsViewer (the SVG), Warn
src-tauri/src/
  main.rs          entry point, calls universalkvm_lib::run()
  lib.rs           Tauri setup, command handlers, tray, focus routing
  networking.rs    libp2p: discovery, pairing, request/response, streams
  storage.rs       config load and save, file transfer, applying config to the window
  states.rs        backend global state and every serde type
  keyboards.rs     keyboard capture and virtual keyboards
  mouses.rs        mouse capture, virtual mice, border and portal geometry
  device_names.rs  the name a device is shown under, read from the Windows device tree
  focus.rs         which machine currently owns the cursor
  clipboard.rs     clipboard read and write
  login.rs         login and lock screen handling
  common.rs        shared helpers
```

### How the halves talk

The frontend calls Rust with `invoke(...)`, and Rust pushes state back with events. There
is no request/response for state: **the frontend never assumes a call succeeded, it waits
for the event.**

Commands (`tauri::generate_handler!` in `lib.rs`):

```
get_config_path          frontend_ready
refresh_self_app         refresh_discovered_apps
submit_app_network_config submit_edit_monitors
set_self_online          set_focused_id           request_clipboard
connect_to_app           disconnect_from_app      submit_config
update_keyboards         update_mouses
refresh_keyboards        refresh_mouses           transfer_files
```

Events emitted to the frontend:

| Event | Carries |
| --- | --- |
| `to-frontend-update-self-app` | this machine's `App` |
| `to-frontend-update-discovered-apps` | `App[]` seen on the network |
| `to-frontend-update-borders` | `BorderPair[]` |
| `to-frontend-update-keyboard-devices` | keyboard list |
| `to-frontend-update-mouse-devices` | mouse list |
| `backend-update-configuration` | persisted settings |
| `backend-add-log` | one log line |

Most commands take a **JSON string**, not an object, for example
`invoke("submit_config", { partial_config: JSON.stringify({ theme }) })`. Match that
convention when adding one.

`App.tsx` owns logs and settings in `GlobalContext`; pages own their own view state.

## Conventions

**Never log secrets.** Passwords travel through `to-frontend-update-self-app` and
`to-frontend-update-discovered-apps`. Those listeners deliberately do not call `debug()`,
and the commented-out debug lines are left in place as a reminder. Logs must never
contain keystrokes or mouse movement either.

**Refresh calls need a tick.** Calling a `refresh_*` command synchronously inside an
effect does not work reliably; the existing code wraps it in
`new Promise(f => setTimeout(f, 0)).then(...)`. Keep that.

**Do not touch the monitor geometry maths** in `mouses.rs` and `MonitorsViewer.tsx`
unless that is the task. Borders, portals, and overlap detection are interdependent.

**An idle device answers with an empty list, not an error.** `get_recent_events` returns
`Ok(vec![])` when nothing happened on Windows and macOS; only Linux returns an error. The
fetch loop runs every millisecond, so anything in it that reads a successful poll as an
event runs a thousand times a second. That is what made the app freeze on Windows and
macOS while Linux looked fine.

**The fetch loop is only as fast as the platform timer.** Its `sleep(1 ms)` really takes
about 15.6 ms on Windows, because that is the timer the OS hands out by default, which put
forwarded mouse movement at 64 Hz and made the cursor stutter on the machine being driven.
`ask_for_a_high_resolution_timer` in `common.rs` asks for 1 ms at startup and brings the
sleep to about 2.6 ms. Measure before assuming a sleep in this loop does what it says.

**Nothing in the fetch loop may reach the Tauri main thread.** `app_handle.cursor_position()`
and `set_cursor_position` post a message to the event loop and block for the answer, so
calling them at loop rate floods the thread that draws the window. The border check runs
on its own thread for that reason, and `IS_CHECKING_BORDER` keeps one check in flight at a
time.

**`xavkeyboardandmousegrabber` is an external crate** from crates.io that provides the
low level device access. Its name is not ours to change.

**Device names are resolved before use.** On Windows the crate reports the driver class,
so every keyboard arrives as "HID Keyboard Device". `device_names.rs` replaces that with
the product name, and `keyboards.rs` and `mouses.rs` apply it to both the device listing
and the opened device, because a device is keyed on its name plus its path. Remembered
devices in the config are therefore matched on the path alone: a resolved name can change
between versions of this app, a path cannot.

## Design system

Dark is the base theme. `src/App.css` defines the palette on `:root`, and the light
palette overrides it under `@media (prefers-color-scheme: light)`. The Rust side drives
that media query through `window.set_theme` in `storage.rs`, from the user's `theme`
setting. Adding a colour means adding a token in **both** blocks.

Colour tokens are stored as space separated channels (`--brass: 224 166 75`), so they
work in both `rgb(var(--brass))` and `rgb(var(--brass) / 0.3)`.

Two accents, each with one fixed meaning. Do not reuse them for decoration:

* `--brass` — this machine, and anything the user can act on.
* `--signal` — a live connection, and the screen that currently holds the cursor.

`--alert` is errors, `--caution` is inline warnings.

Typography: **Space Grotesk** for interface text, **JetBrains Mono** (`.mono`,
`--font-data`) for anything the machine produced — peer ids, addresses, resolutions,
paths, byte counts, timestamps. Both are bundled through `@fontsource-variable/*` rather
than fetched, because the app must work offline.

State is never carried by colour alone. Status lamps always sit next to words, and error
log lines get a `!` in the gutter.

SVG presentation attributes cannot read CSS custom properties. Anything in
`MonitorsViewer.tsx` that must follow the theme uses a class defined in
`MonitorsViewer.css`; the per-monitor colours stay as attributes because they come from
the backend.

## Releasing

Version lives in four places and they must agree:
`package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (the `universalkvm`
entry), and `src-tauri/tauri.conf.json`.

Then tag it:

```sh
git tag v1.2.3 && git push origin v1.2.3
```

`.github/workflows/release.yml` builds macOS (both architectures), Linux, and Windows
installers and opens a **draft** release with them attached. Review the notes and
publish it by hand. Tauri bundles native packages, so each platform must build on its
own runner; there is no cross-compiling these.

### Code signing

The macOS jobs set `APPLE_SIGNING_IDENTITY=-`, which ad-hoc signs the bundle during
bundling. **Do not remove it.** Without it Tauri leaves the app linker-signed with its
resources unhashed, and macOS reads that as a broken signature: a downloaded copy is
refused with *"UniversalKVM is damaged and can't be opened"* rather than the ordinary
unidentified-developer prompt. A verification step runs `codesign --verify --strict`
and fails the build if the signature is not valid, so this cannot regress unnoticed.

Ad-hoc signing only makes the signature well formed. It is not an Apple Developer ID and
not notarization, so Gatekeeper still does not trust the app: users either right-click →
Open or clear the quarantine flag, both covered in the README. Real trust needs a paid
Apple Developer account and `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` as repository
secrets. The Windows installers are unsigned, so SmartScreen warns on first run.

## Icon and logo

Both come from the same mark: two cables crossing, one keyboard and mouse swapping
between two machines.

* `src-tauri/icons/source-icon.svg` — the app icon source, mark on a rounded plate.
  Every file in `src-tauri/icons/` is generated from it:

  ```sh
  pnpm tauri icon src-tauri/icons/source-icon.svg
  ```

  Edit the source and rerun that; never hand-edit the generated PNGs, `.icns`, or `.ico`.

* `public/universalkvm.svg` — the same mark without the plate, used in the top bar and on
  the About tab. Its colours are literal rather than `currentColor`, because it loads
  through an `<img>` tag and cannot inherit the page's text colour.

The mark is deliberately down to two shapes so its silhouette survives at 16 px.

## Identifiers you must not change casually

Four strings are load-bearing. Changing any of them breaks existing installs, and each
needs a migration path if it ever has to move:

| What | Value | Breaks if changed |
| --- | --- | --- |
| Bundle identifier | `com.universalkvm.app` | macOS accessibility permission has to be granted again |
| Config directory | `<config dir>/universalkvm` | settings are no longer found |
| Config file | `universalkvm_config.txt` | settings are no longer found |
| libp2p protocol | `/universalkvm` | peers on the old string stop pairing; every machine must upgrade together |

The protocol string is the harshest of the four: it must match on both ends of every
connection, so a change forces a simultaneous upgrade across a user's whole desk.
