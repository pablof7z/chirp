# Chirp

A Nostr client for iOS, Android, TUI, and desktop, built on [NMP](https://github.com/pablof7z/nostr-multi-platform).

## Platforms

| Platform | Location | Build |
|---|---|---|
| iOS | `apps/ios/` | Xcode |
| Android | `apps/android/` | Gradle |
| TUI | `apps/tui/` | `cargo run -p chirp-tui` |
| Desktop | `apps/desktop/` | `cargo run -p chirp-desktop` |
| Web | `apps/web/` | `npm run dev` |

## iOS

Open `apps/ios/Chirp.xcodeproj` in Xcode. The Rust static libraries are built by the Xcode build phase via `just build-ios` (see `ios/project.yml`).

Requires Xcode 16+ and `IPHONEOS_DEPLOYMENT_TARGET=17.0`.

## Android

```sh
cd apps/android
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
cd apps/web
npm install
npm run dev
```
