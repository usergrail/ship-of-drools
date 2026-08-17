```
 ⌜                                                      ⌝
    ██████╗ ██████╗  ██████╗  ██████╗ ██╗     ███████╗
    ██╔══██╗██╔══██╗██╔═══██╗██╔═══██╗██║     ██╔════╝
    ██║  ██║██████╔╝██║   ██║██║   ██║██║     ███████╗
    ██║  ██║██╔══██╗██║   ██║██║   ██║██║     ╚════██║
    ██████╔╝██║  ██║╚██████╔╝╚██████╔╝███████╗███████║
    ╚═════╝ ╚═╝  ╚═╝ ╚═════╝  ╚═════╝ ╚══════╝╚══════╝
 ⌞                                                      ⌟
              - A masterpiece made by grail. -
```

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.78+-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/platform-windows-blue?style=flat-square&logo=windows)
![Requires Admin](https://img.shields.io/badge/requires-administrator-red?style=flat-square)
![Game](https://img.shields.io/badge/game-Ship%20of%20Fools-teal?style=flat-square)

</div>

---

An external memory trainer for **Ship of Fools** written in Rust. Attaches to the live game process and lets you read and write squids, sand dollars, and harpoons directly through pointer chains resolved at runtime. No DLL injection, no hooks — pure external read/write via `ReadProcessMemory` and `WriteProcessMemory`.

This is my first reverse engineering project. I wanted to document the process properly and put it out there rather than leaving it sitting on my machine. If anything here is wrong, could be done better, or you just want to talk about the approach — feel free to open an issue or message me. I'm actively trying to learn this space and genuinely welcome the feedback.

---

## How It Works

Ship of Fools runs on Unity IL2CPP, which means all game objects live on a managed heap with addresses that change every session. The trainer resolves a **stable pointer chain** anchored to a static offset inside `GameAssembly.dll` — the only address that doesn't move between restarts.

```
GameAssembly.dll + 0x030B5818
  → +0xB8  → +0x08  → +0x10  → +0x170   ← container object (stable)
    → +0xA8  → +0x80  → +0x3F8           ← squids
    → +0xA8  → +0x80  → +0x6C8           ← sand dollars
GameAssembly.dll + 0x032559E8
  → +0x190  → +0x1E0  → +0xE8            ← harpoons
```

Every time the game starts, the heap moves — but the chain of relative offsets through the object graph stays the same. The trainer walks this chain on each access to compute the current live address.

---

## Features

| Resource | Read | Write |
|---|---|---|
| Squid Tokens | ✓ | ✓ |
| Sand Dollars | ✓ | ✓ |
| Harpoons | ✓ | ✓ |

---

## Requirements

- **Administrator** — Windows will deny `OpenProcess` on a protected game without elevated privileges
- **Ship of Fools must be running** before you launch this — it will exit immediately if the process isn't found

---

## Usage

Launch `ship-of-drools.exe` as administrator while the game is open.

```
 ⌜                                                      ⌝
   Current Values:

   Squids:         9995
   Sand Dollars:   415
   Harpoons:       199

   Commands:

   print      | Print current values
   set        | Set values
   exit       | Quit
 ⌞                                                      ⌟

 >
```

Type `set` and enter values at each prompt. Leave any field blank to skip it. Type `exit` to close.

---

## Scan Mode

If an update breaks the pointer paths, use scan mode to find new offsets:

```
ship-of-drools.exe scan <current_value>
```

Pass your current in-game count as the argument. It brute-forces 3-hop paths from all known static entry points and prints every match:

```
[*] Pass 1 — from squid-token container 0x000001EEDBF43C0
[+] container+0xA8→+0x80→+0x3F8 = 9995

[*] Pass 2 — dll+0x032559E8
[+] dll+0x032559E8→*+0x190→+0x1E0→+0xE8 = 199
```

Update the corresponding offsets in `src/main.rs` and rebuild.

---

## Build

Requires the [Rust toolchain](https://rustup.rs) and the `x86_64-pc-windows-msvc` target.

```sh
cargo build --release
```

Output: `target/release/ship-of-drools.exe`

---

## Notes

- Paths are pinned to a specific game version — run scan mode after updates
- Harpoons route through a direct DLL static rather than the container chain and may occasionally need a second write to stick
- The diagnostic module (`src/diagnostic.rs`) contains a raw memory walker for reverse engineering new object layouts — not used at runtime
