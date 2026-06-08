# Receiver

Receiver is an internet radio project with two parts:

- a GTK/Libadwaita desktop app written in Vala
- `receiver-core`, a Rust library that ports the non-UI technical logic

The desktop app is still the runnable user-facing application. The Rust crate is
the reusable core for station data, metadata cleanup, favourites, history,
Last.fm, image caching, and stream URL resolution. It does not start audio
playback itself.

![Receiver - browsing stations](data/banner.png)

## Project Status

The repository currently keeps the original Vala app and adds a Rust workspace
for the technical core.

`receiver-core` is intentionally a library, not an executable. It exposes the
logic needed by a future app or integration layer, while avoiding GTK,
Libadwaita, GObject, MPRIS, and audio pipeline ownership.

Implemented in Rust:

- station model and best stream selection
- read-only SQLite access to `data/receiver/receiver.db`
- station search and filters using the existing database schema
- favourites and application state JSON persistence
- song history JSON persistence
- ICY metadata parsing, cleanup, and artist/title extraction
- image cache download and lookup
- Last.fm signing, auth, now-playing, and scrobble requests
- scrobbling state machine
- logical player/session state with playlist and redirect resolution

Not included in `receiver-core`:

- GTK/Libadwaita UI
- GStreamer or any other audio pipeline
- MPRIS desktop integration
- YouTube download support
- Vala/C FFI bindings

## Repository Layout

```text
src/                    Vala GTK application
src/models/             Vala domain models
src/services/           Vala technical services
src/utils/              Vala parsing and utility logic
src/widgets/            GTK/Libadwaita UI widgets
crates/receiver-core/   Rust non-UI library
data/                   app metadata, icons, schemas, bundled station DB
po/                     translations
debian/                 Debian packaging
snap/                   Snap packaging
subprojects/ytdl/       bundled Vala YouTube helper
```

## Rust Core

Build and test the Rust library:

```sh
cargo test -p receiver-core
```

Format check:

```sh
cargo fmt --all --check
```

The Rust crate uses `rusqlite` with bundled SQLite, so tests can open the
bundled station database without requiring the `sqlite3` CLI.

Public modules:

- `models` - station, stream, track, and player state data types
- `stations` - SQLite repository and station filtering
- `state` - favourites and settings persistence
- `history` - recently played song persistence
- `metadata` - ICY metadata cleanup and artist/title extraction
- `images` - image cache and download helper
- `lastfm` - Last.fm API client
- `scrobbler` - scrobble timing and state
- `player` - logical station session and stream URL resolution

## Desktop App

Build the existing Vala app:

```sh
make build
```

Run it from the repository:

```sh
make run
```

Clean build output:

```sh
make clean
```

Translation validation:

```sh
make translations-check
```

Packaging helpers:

```sh
make deb
make appimage
```

## Install

### Flathub

[![Get it on Flathub](https://flathub.org/api/badge)](https://flathub.org/apps/io.github.meehow.Receiver)

```sh
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install flathub io.github.meehow.Receiver
flatpak run io.github.meehow.Receiver
```

### Snap Store

[![Get it from the Snap Store](https://snapcraft.io/static/images/badges/en/snap-store-black.svg)](https://snapcraft.io/receiver)

```sh
sudo snap install receiver
```

### Debian / Ubuntu

Download a `.deb` package from the releases page and install it with:

```sh
sudo apt install ./receiver_*.deb
```

## Development Notes

- The Vala app remains independent from the Rust crate for now.
- `receiver-core` preserves the existing station database and user data shapes
  where they are already represented in Rust.
- Build artifacts such as `builddir/` and `target/` are ignored.
- See [CONTRIBUTING.md](CONTRIBUTING.md) for the broader development setup.

## License

Receiver is licensed under GPL-3.0-or-later. See [LICENSE](LICENSE).
