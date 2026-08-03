# pmdmini-rs

GUI player for PC-98 PMD music files (.M/.M2/.M26/.M86) on Windows, Linux and browsers.

## Native build

Install Rust and the development packages required by your window and audio
backend. On Debian or Ubuntu, ALSA headers are usually enough.

Run the checks and start the application from the repository root:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p pmd-app --release
```

Press `FILE` to choose a main `.M`, `.M2`, or `.MZ` file. Companion files are
read from the same directory. The loader accepts banks such as `.PPC`, `.PPS`,
`.PZI`, `.PVI`, and `.P86`, without depending on filename case.

Files and folders can also be dropped on the window. 

## Browser build

Install the WebAssembly target and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

Start a development server:

```sh
trunk serve
```

Build the release bundle and serve it locally:

```sh
trunk build --release
python3 -m http.server 8000 --directory dist
```

Open <http://127.0.0.1:8000/> in a browser. Use `FILE` or drop the main file
and its companion banks on the page. The browser keeps the selected files in
memory and does not access the local filesystem. Press `PLAY` after loading a
file so the browser can start Web Audio from a user gesture.

## Controls

The main controls are painted into the monitor:

- `PLAY`, `STOP`, `PAUSE`, `REW`, `FF`, `PREV`, and `NEXT` control playback;
- `LIST` opens the playlist;
- clicking a channel header mutes it;
- `LOOP COUNT、` changes the loop limit;
- dropping files adds them to the playlist.

Useful keyboard shortcuts are `Space` for play/pause, `Enter` to play the
selected entry, `N` and `P` for next/previous, `L` for the playlist, and `F11`
for fullscreen.

Native settings are saved to `$XDG_CONFIG_HOME/pmdmini/settings.conf`, or to
`$HOME/.config/pmdmini/settings.conf` when `XDG_CONFIG_HOME` is not set. The
browser uses `localStorage`.

## Verification

The usual local check is:

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p pmd-app --target wasm32-unknown-unknown
```

## License

Project-authored source is distributed under the GNU Affero General Public
License, version 3.0 only (`AGPL-3.0-only`). `NOTICE` records the upstream and
third-party attributions and terms that also apply to parts of the project.
