# Bongo Cat Input Overlay

A lightweight desktop widget that displays a Bongo Cat tapping its paws in real-time as you type.

## Demonstration
Tested on **niri** compositor.

https://github.com/user-attachments/assets/f15b4fd8-ef80-43ce-944c-d3116e5d38bc


## Installation

### Release downloads

Download and extract the archive for your platform from
[Releases](https://github.com/gh0stErr0r/bongo/releases):

- `bongo-linux-musl-x86_64.tar.gz`
- `bongo-macos-x86_64.tar.gz` (Intel)
- `bongo-macos-arm64.tar.gz` (Apple Silicon)

Cat images are embedded in the binary. Custom images can still be configured.
The Linux binary is built statically for `x86_64-unknown-linux-musl`. It uses
Wayland directly and does not require GTK, a musl installation, or shared
libraries. A running Wayland compositor with the wlr-layer-shell protocol
(such as niri, Sway, or Hyprland) is required. On macOS, install GTK4. Input
permissions are described below.

`SHA256SUMS` contains checksums for the binary archives. GitHub also provides
source code archives in ZIP and tar.gz formats.

### Prerequisites
Linux builds need only Rust and Cargo. The Wayland protocol client, image
decoders, and renderer are built into the executable. No system GTK or Wayland
development packages are needed. PNG, JPEG, GIF, BMP, ICO, TIFF, and WebP custom
images are supported (animated images use their first frame).

For macOS (GTK 4.8 or newer):
```bash
brew install gtk4 pkg-config
```

### Building from source
Clone the repository and run:
```bash
cd /path/to/bongo
cargo build --release
```

To build the standalone Linux release binary:

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --locked --release --target x86_64-unknown-linux-musl
```

CI rejects a Linux release if `readelf` finds an ELF interpreter or any `NEEDED`
shared libraries. Publishing a GitHub release builds the three platform archives
from its tag and attaches them, along with `SHA256SUMS`, after all builds pass.

On Linux, the binary can be installed with `sudo cp ./target/release/bongo /usr/bin`.

> **Linux note:** Since the app reads directly from `/dev/input/`, you might need to run it with `sudo` or add your user to the `input` group: `sudo usermod -aG input $USER` (relogin required).

### macOS setup (the macos is ai slopped. You can burn your computer or install asahi)

1. Install GTK 4.8 or newer and `pkg-config` using the command above. Make sure
   Rust and Cargo are installed, then build and launch from the repository:

   ```bash
   cargo build --release
   ./target/release/bongo
   ```

2. Allow keyboard monitoring when prompted. If you see
   `macOS denied keyboard access`, open **System Settings → Privacy & Security →
   Input Monitoring** .

3. Enable the application you use to launch Bongo, such as **Terminal** or
   **iTerm**. If it is missing, click **+** and add it from Applications.
   When launching through an IDE's integrated terminal, enable that IDE instead.
   If macOS lists **bongo** itself, enable that entry too.

4. Quit the terminal or launcher completely with **⌘Q**, reopen it, and run
   `./target/release/bongo` again. Closing just the terminal tab is not enough.
   The permission error should disappear, and the cat should react to typing
   even when another application is focused.

To choose the corner, create `~/.config/bongo.toml`:

```bash
mkdir -p ~/.config
```

```toml
[window]
# top-left, top-right, bottom-left or bottom-right
position = "top-right"
# Cat's longest side in logical pixels; aspect ratio is preserved.
size = 64
```

Restart Bongo after changing the configuration. The widget stays above ordinary
windows on all desktops, with a 16-pixel margin from the usable screen edges
(excluding the menu bar and Dock). Bundled cat images are used by default;
see below to supply your own. On macOS, keyboard events are observed globally,
so the `[input]` section is unnecessary and `device` is ignored.

## Configuration
Create a configuration file at `~/.config/bongo.toml`:

```toml
[imgs]
rest = "~/.config/bongo/cat-rest.png"
left = "~/.config/bongo/cat-left.png"
right = "~/.config/bongo/cat-right.png"

[input]
device = "AT Translated Set 2 keyboard"

[window]
# top-left, top-right, bottom-left or bottom-right
position = "top-left"
# Cat's longest side in logical pixels (1–2048); default: 64.
size = 64
```

The cat is shown on a transparent background without a frame or padding.
Change `[window].size` to resize it; its aspect ratio is preserved. The setting
uses logical pixels, so it has the same apparent size on HiDPI screens. Restart
Bongo after editing the configuration.
