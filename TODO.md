# Hearth: to do

State on 2026-11-26: v0.1.2 is released on GitHub (.deb, .rpm, signed APK). Linux packages and the setup wizard are tested on Ubuntu 24.04, Fedora 43 (container) and Arch.

## Fix first

- [ ] **Web filtering card shows made-up numbers.** The Today screen says "SafeSearch on · 4 sites blocked today" from demo data (`app/lib/screens.dart:320`). Either have `hearthd` count blocked lookups per child and send them with the status, or hide the card until it does.

## Publishing

### AUR (on hold)
- [ ] Upload `packaging/aur/PKGBUILD` and `hearth.install`. Needs an AUR account with an SSH key; then `makepkg --printsrcinfo > .SRCINFO`, commit and push to `ssh://aur@aur.archlinux.org/hearth.git`.
- [ ] Bump `pkgver` with each release (the source is the `v$pkgver` git tag).

### F-Droid
- [x] Scanner without Google ML Kit (`flutter_zxing`, since 0.1.1).
- [ ] Build recipe for `fdroiddata` (`metadata/org.hearth.hearth.yml`): Flutter from source, the Rust toolchain for `app/rust` (built through `rust_builder`), Android NDK version, `flutter build apk --split-per-abi` or a universal APK.
- [ ] Check the build is reproducible, or let F-Droid sign it.
- [ ] Store listing in the repo (`fastlane/metadata/android/en-US/`): title, short and full description, changelogs, screenshots, the 512 px icon (generated from `app/assets/icon/icon.svg`).
- [ ] Anti-features: none expected. The default relay is ntfy.sh (a free service) and the relay can be changed. Confirm with the F-Droid reviewers.
- [ ] Open the merge request on gitlab.com/fdroid/fdroiddata.

### Google Play
- [ ] Developer account (one-off fee; new personal accounts need a closed test with 12 testers for 14 days before production).
- [ ] Build an app bundle: `flutter build appbundle` (signs with the upload key in `~/.config/hearth-signing/`, never committed).
- [ ] Store listing, privacy policy page, Data safety form (no data collected; messages end to end encrypted through the relay), content rating.

## Not tested yet
- [ ] The wizard's warning when GNOME's own parental controls (Malcontent) also limit a child: needs a GNOME machine with session limits set.
- [ ] The .rpm on a real Fedora install (services starting; the container had no systemd).
- [ ] Fedora's `authselect` may write `nss-resolve` back into `/etc/nsswitch.conf`, reopening the web-filter bypass. `hearthd` could check this at start (`crates/hearthd/src/setup.rs:196`).

## Later (deliberate shortcuts, upgrade when needed)
- [ ] Name the phone when pairing; it's always "Parent's phone" (`app/lib/hub.dart:188`).
- [ ] A display name per child, editable in the app (`app/lib/hub.dart:251`).
- [ ] Per-app limits: pick from the PC's installed apps instead of typing the app ID (`app/lib/screens.dart:1335`).
- [ ] Notifications without a UnifiedPush app (embedded FCM), only if parents ask (`app/lib/push.dart:19`).
- [ ] Per-day rules beyond school days / weekend; the TOML import drops other days (`crates/hearthd/src/config.rs:98`).
- [ ] Idle timeout per child (`crates/hearthd/src/accounting.rs:4`).
- [ ] Desktop admin through polkit instead of a root check (`crates/hearthd/src/api.rs:59`).
- [ ] Count relay messages that fail to open instead of dropping them silently (`crates/hearth-core/src/client.rs:138`).
- [ ] Filter private addresses for phone push endpoints (`crates/hearthd/src/relay.rs:224`).
- [ ] iOS app: no `ios/` project yet (needs a Mac).
