# kitbar-rust

A bar in Kitty for Hyprland.

kitbar-rust uses Hyprland's special workspace `special:kitbar` to provide a lightweight, integrated status bar experience within the Kitty terminal.

## Features

- Runs as a bar inside Kitty terminal on Hyprland
- Uses Hyprland special workspace for seamless integration
- Encrypted IPC via AES-GCM with X25519 key exchange
- XDG-compliant configuration directory
- Terminal color and style support
- Monitor-only mode for dedicated display

## Hyprland Configuration

Add the following to your `hyprland.conf`:

```ini
# new workspace
workspace = special:kitbar, persistent:true

# keybind is $mainMod + space
bind = $mainMod, space, togglespecialworkspace, kitbar

# open
exec-once = ~/app/kitbar &
```

## Usage

Run kitbar:

```bash
kitbar
```

If you want to use a single monitor only:

```bash
kitbar --monitor
```

## Build

```bash
cargo build --release
```

The binary will be available at `target/release/kitbar`.

## Dependencies

Key Rust crates used:

| Crate | Purpose |
|---|---|
| aes-gcm | AES-GCM authenticated encryption |
| x25519-dalek | Curve25519 Diffie-Hellman key exchange |
| chrono | Date and time handling |
| base85 | Base85 encoding |
| bon | Builder pattern macros |
| xdg | XDG base directory specification |
| anstream / anstyle | Terminal styling and colors |
| winnow | Parser combinator |
| serde | Serialization framework |
| zeroize | Secure memory zeroing |

## License

Please refer to the project source for license information.
