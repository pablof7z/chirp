# Chirp

A Nostr client for iOS, Android, TUI, and desktop, built on [NMP](https://github.com/pablof7z/nostr-multi-platform).

## Platforms

| Platform | Location | Build |
|---|---|---|
| iOS | `ios/` | Xcode |
| Android | `android/` | Gradle |
| TUI | `chirp-tui/` | `cargo run -p chirp-tui` |
| Desktop | `chirp-desktop/` | `cargo run -p chirp-desktop` |
| Web | `web/` | `npm run dev` |

## Local development

Chirp depends on [NMP](https://github.com/pablof7z/nostr-multi-platform) via path dependency. Clone both repos as siblings:

```sh
git clone https://github.com/pablof7z/nostr-multi-platform
git clone https://github.com/pablof7z/chirp
```

The `Cargo.toml` workspace references NMP at `../nostr-multi-platform`.

## iOS

Open `ios/Chirp.xcodeproj` in Xcode. The Rust static libraries are built by the Xcode build phase via `just build-ios` (see `ios/project.yml`).

Requires Xcode 16+ and `IPHONEOS_DEPLOYMENT_TARGET=17.0`.

## Android

```sh
cd android
./gradlew assembleDebug
```

The Rust `cdylib` is built by the Gradle `cargoNdk` task before assembly.

## TUI / Desktop

```sh
cargo run -p chirp-tui
cargo run -p chirp-desktop
```

## Web

```sh
cd web
npm install
npm run dev
```
