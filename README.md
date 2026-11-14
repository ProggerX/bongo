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
The Linux binary is built for `x86_64-unknown-linux-musl`. It requires the musl
loader and GTK4 / GTK4 Layer Shell libraries built for musl; these are not
included in the archive. On macOS, install GTK4. Input permissions are described
below.

`SHA256SUMS` contains checksums for the binary archives. GitHub also provides
source code archives in ZIP and tar.gz formats.

### Prerequisites
Before building from source, install GTK4. Linux also requires GTK4 Layer Shell.

For Arch Linux:
```bash
sudo pacman -S gtk4 gtk4-layer-shell
```

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
```
