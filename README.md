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

An external memory trainer for **Ship of Fools** written in Rust. Attaches to the live game process and lets you read and write squids, sand dollars, and harpoons through the game's own IL2CPP runtime structures. No DLL injection, no hooks — pure external read/write via `ReadProcessMemory` and `WriteProcessMemory`.

This is my first reverse engineering project. I wanted to document the process properly and put it out there rather than leaving it sitting on my machine. If anything here is wrong, could be done better, or you just want to talk about the approach — feel free to open an issue or message me. I'm actively trying to learn this space and genuinely welcome the feedback.

---

## How It Works

Ship of Fools runs on Unity IL2CPP, so addresses and layouts can move between sessions or builds. The trainer reads the remote `GameAssembly.dll` PE export table and uses the game's own `il2cpp_*` accessors to recover its domain, assemblies, images, classes, static storage, field lists, field names, and field offsets at runtime.

```
GameAssembly exports
  → IL2CPP domain → assembly vector → Core.dll image
  → Impulser class → FieldInfo metadata → static field data
  → Impulser instance → BoatManager → GameState
    → shardsCount NetworkVariable           ← squids
    → cashCount NetworkVariable             ← sand dollars
    → harpoonMagazine                       ← harpoons
```

There are no compiled game addresses, pointer paths, native IL2CPP structure offsets, or managed field offsets in the active code. The only game-specific identifiers are assembly, class, and field names. Fixed numbers inside the resolver belong to the PE32+ format, x64 instruction encoding, and bounded safety checks.

This makes it an automatic updater for layout changes, not a promise that every future build will work untouched. Address randomization, relocated globals, and changed field offsets are handled automatically. Renamed or removed classes and fields, removed exports, or a major change to the generated accessor code can still require an update.

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
- **Ship of Fools must be running** before you launch this
- The trainer waits up to 60 seconds for IL2CPP and the gameplay objects to finish initializing

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

## Build

Requires the [Rust toolchain](https://rustup.rs) and the `x86_64-pc-windows-msvc` target.

```sh
cargo build --release
```

Output: `target/release/ship-of-drools.exe`

---

## Notes

- Native and managed field layouts are resolved from the running game's IL2CPP exports and metadata
- Harpoons use the live `Magazine` count selected by the same state flags as the game's getter
- Resource writes still use `WriteProcessMemory`; the IL2CPP path resolves where to write
