# Changelog

## v1.1.0 - 2026-08-18

- replaced fixed `GameAssembly.dll` roots and gameplay pointer offsets with runtime IL2CPP export resolution
- added remote PE32+ export lookup with x64 thunk following
- derived the domain, assembly image, class table, static storage, managed field list, field names, and field offsets from the running game
- resolved squids, sand dollars, and harpoons through live object classes and `FieldInfo` metadata
- added a 60-second attach window while IL2CPP and gameplay objects initialize
- removed the legacy hardcoded scanner, pointer-chain, and diagnostic paths
- kept only semantic assembly, class, and field names as game-specific identifiers

## v1.0.0 - 2026-08-17

- initial release
