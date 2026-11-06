# PSU
[![Chat on discord](https://img.shields.io/discord/1183477399208345771?color=blue8&label=Discord&logo=discord&style=for-the-badge)](https://discord.gg/qtjrKnCk8y)

---

## What is this

PSU is a terminal (TUI) password manager. Passwords are stored in a local
SQLite database and encrypted with XChaCha20-Poly1305; the encryption key is
derived from a master password using Argon2id. Secrets are zeroized in memory
(`secrecy` / `zeroize`) where possible.

## Quickstart

```
cargo build --release
./target/release/psu
```

On the first run you will be asked to create a master password. On subsequent
runs you unlock the database with it. If a database from the old plaintext
CSV version (`psu.csv`) is found, its records are imported automatically and
the CSV is renamed to `psu.csv.bak`.

Data is stored in `psu.db` next to the path from the config file
(`~/.config/psu/config.toml` by default, supports `~` expansion).

## Keybindings

### Global
- `Esc` — quit / close popup
- `n` — new password
- `m` — modify selected password
- `d` — delete selected password
- `Enter` — save (in the form popup)
- `c` — copy selected column to clipboard
- `p` — copy password to clipboard
- `P` — copy whole row to clipboard
- `?` — help

### Navigation
- `↑` / `k`, `↓` / `j` — move between rows
- `←` / `h`, `→` / `l` — move between columns
- `Tab` / `↓`, `↑` — switch fields in the form popup

---

## Assembling

You must have Rust installed, then clone and build:

```
git clone https://github.com/IwantHAPPINESS/psu.git
cd psu
cargo build --release
```

The binary is `target/release/psu`.
