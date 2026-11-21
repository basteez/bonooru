# Bonooru app: language and GUI toolkit evaluation

Date: 2026-11-19. Status: research input to a possible reopening of ADR-005 (accepted, Python) and to ADR-008 (proposed, toolkit). Nothing here is decided until the owner accepts it.

Scope: the desktop app in `app/` (the notes call it "host"). Requirements used: FR-01…FR-15, NFR-01, NFR-07, NFR-10, R5, R12, ADR-003 (the device schedules notes, so the app is not real time) and ADR-007 (live MIDI after v1).

Evidence levels used below: **measured** (built here), **checked** (registry, repo or official docs, URL in Sources), **estimate** (my judgement, not verified).

---

## 1. Summary and recommendation

**Top pick: Rust + egui/eframe**, with `serialport`, `midly`, `midir`, `rodio`/`cpal`, `rfd`, `sysinfo`. **Runner-up: Python + PySide6**, if the owner relaxes NFR-07 to allow LGPL with dynamic linking (Qt and, if you use QtMultimedia, FFmpeg).

**Is reopening ADR-005 worth it? Yes, as a time-boxed spike, not as a decision by default.** No app code exists yet (M0/M1 done, M2+ to do), so switching costs nothing today and a lot after M6. ADR-005 rejected Rust only because it is "slower to iterate". That is true, but the hard problems in this project are **distribution, first-run trust and licences** (NFR-01, NFR-07, NFR-10, R5, R12), not UI iteration speed. These are exactly the areas where Python is weakest and Rust is strongest.

Decisive reasons:

1. **Distribution matches NFR-01/NFR-10.** A Rust app is one native executable per OS, with no bundler that unpacks itself and no interpreter inside. PyInstaller output is a folder (onedir, needed anyway for LGPL Qt replaceability) or a self-extracting onefile that starts slowly and is a known trigger for antivirus heuristics. Measured here (macOS arm64, Python 3.12, PyInstaller 6.22): a PySide6 skeleton with QtWidgets, QtSerialPort and QtMultimedia is **113 MB on disk / 42 MB zipped**. A CustomTkinter + numpy + sounddevice skeleton is **41 MB / 19 MB**. An eframe release binary is typically in the single-digit to low tens of MB (estimate, not measured: no Rust toolchain on this machine).
2. **Licence fit without bending NFR-07.** egui, eframe, rfd, midir, rodio, cpal, sysinfo and nusb are MIT and/or Apache-2.0, and midly is Unlicense. The one exception is `serialport` (**MPL-2.0**, checked). MPL is file-level copyleft: static linking is allowed and you only have to point to the unmodified upstream source. That is a much smaller change to NFR-07 than LGPL Qt. If NFR-07 must stay literal, use `serial2` (BSD-2/Apache) and do VID/PID lookup yourself, which is more work. PySide6 requires an LGPL relaxation, and QtMultimedia also ships FFmpeg dylibs (LGPL), measured in the bundle.
3. **Every functional need is covered by mature crates, and the UI model fits this app.** egui's `Painter` is a direct fit for a piano roll. `ctx.input()` gives key down/up with an explicit repeat flag, which suits a hold-to-press on-screen gamepad. `request_repaint()` can be called from the serial worker thread (checked in egui source). Native file dialogs come through `rfd`. The app has about five screens and one custom canvas, a size where immediate mode stays simple.
4. **Project health is strong.** egui/eframe 0.37.0 came out on 2026-10-15 and released monthly through 2026. serialport 4.10.1 came out on 2026-09-08 and sysinfo 0.40.4 on 2026-11-16 (checked).

Honest costs of the top pick:

- **Learning curve.** If the owner does not already write Rust, expect 2–4 weekends before the speed matches Python (estimate). The C firmware work helps with the concepts, not with the borrow checker.
- **Less polish out of the box.** egui does not look native, has no ready-made wizard widget and no rich text widgets. The flashing wizard is a hand-written state machine, which is fine at this size.
- **Pre-1.0 churn.** egui breaks its API on minor versions (0.x). Pin the version and upgrade on purpose.
- **Code signing is still needed.** Rust does not remove R5. It lowers antivirus noise, but SmartScreen reputation and Gatekeeper do not depend on the language (see §5).

**When not to switch.** If the spike shows Rust slows the owner down so much that M2–M6 slip, or if the owner wants native widgets and a ready-made wizard more than a small download, keep Python, take PySide6 (relax NFR-07) and accept a ~40 MB download plus the PyInstaller antivirus risk. Do not fall back to CustomTkinter for this app (see §3).

**Suggested spike (1–2 weekends, exit criteria):** an eframe app that (a) finds the bridge by VID/PID/product string and shows its INFO, (b) draws a 2,000-note piano roll that scrolls smoothly, (c) plays a sine preview, (d) finds `RPI-RP2` and copies a UF2, (e) builds unsigned artefacts for 3 OS on GitHub Actions. If (a)–(e) are done and still fun, reopen ADR-005 in favour of Rust. If not, accept ADR-008 with PySide6.

---

## 2. Comparison table

Scores 1 (poor) … 5 (excellent) for this project. "Dist" covers size, single file, startup and antivirus. "Lic" is fit with NFR-07 as written. "Libs" covers serial with VID/PID, `.mid` parsing, MIDI input, audio synthesis, drive detection and custom 2D drawing. "X-plat" covers HiDPI, native dialogs and maturity on 3 OS. "Iter" is iteration speed for an intermediate solo dev. "Test" is testability of a GUI-free core. "CI" is 3-OS GitHub Actions builds. "Health" is project activity as of Nov 2026.

| Candidate | Dist | Lic | Libs | X-plat | Iter | Test | CI | Health | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| **Rust + egui/eframe** | 5 | 4 (MPL serialport) | 4 | 4 | 3 | 5 | 5 | 5 | **Top pick** |
| **Python + PySide6** | 2 | 2 (LGPL Qt + FFmpeg) | 5 | 5 | 5 | 5 | 3 | 5 | **Runner-up** |
| Rust + Slint | 5 | 3 (royalty-free licence with attribution) | 3 | 4 | 3 | 5 | 4 | 5 | Viable, piano roll is clunkier |
| Rust + Tauri 2 | 4 | 4 | 4 | 3 (Linux WebKitGTK) | 3 (two languages) | 5 | 4 | 5 | Viable if you like web UI |
| Rust + Iced | 5 | 4 | 4 | 4 | 2 | 5 | 5 | 3 | Steeper; slow releases |
| Rust + Dioxus desktop | 4 | 4 | 3 | 3 | 3 | 4 | 4 | 3 (0.8 alpha churn) | Not yet |
| Python + CustomTkinter | 3 | 5 | 3 | 2 | 4 | 5 | 3 | 3 | Fallback only |
| Python + Dear PyGui | 3 | 5 | 3 | 3 | 4 | 5 | 3 | 3 | Niche |
| Python + Flet | 2 | 5 | 3 | 3 | 4 | 4 | 2 | 4 | Packaging risk |
| Python + Toga | 3 | 5 | 2 | 3 | 3 | 5 | 3 | 4 | Missing pieces |
| Rust + gtk4-rs | 2 | 2 (GTK LGPL) | 4 | 2 (Win/macOS) | 3 | 5 | 2 | 4 | No |
| Rust + Qt (cxx-qt) | 2 | 2 | 5 | 5 | 1 | 4 | 1 | 3 | No |
| Go + Fyne / Wails | 4 | 5 | 4 | 3 | 4 | 5 | 4 | 4 | Credible outsider |
| Electron (reference) | 1 | 4 | 4 | 4 | 4 | 4 | 4 | 5 | Reference only |

---

## 3. Per-candidate notes

### Rust + egui/eframe (top pick)

- **Versions and health (checked):** egui and eframe 0.37.0 came out on 2026-10-15, after 0.34.3 (May), 0.35.0 (Jun), 0.36.0–0.36.2 (Aug–Sep). egui_plot 0.38.0 came out on 2026-10-19. Licence is MIT OR Apache-2.0. About 10 M recent downloads.
- **Serial:** `serialport` 4.10.1 (MPL-2.0). `available_ports()` returns `UsbPortInfo` with VID, PID, serial number, manufacturer and product, so the "Bonooru Bridge" lookup works directly. Setting the baud rate to 1200 and closing the port does the 1200-baud touch. On Linux the build needs `libudev-dev` and the runtime needs `libudev.so.1`, which every mainstream distro has. Alternative: `serial2` (BSD-2/Apache), but its `available_ports()` returns only paths (checked), so VID/PID would have to come from `nusb` (MIT/Apache) plus your own matching per OS.
- **MIDI files:** `midly` 0.5.3 (Unlicense). Its last release was 2023, but the SMF format is frozen and the crate is zero-copy and complete. Low risk. A plain SMF parser is also only a few hundred lines.
- **Live MIDI (after v1):** `midir` 0.11.1 (MIT), released 2026-10-10. It uses WinMM, CoreMIDI and ALSA.
- **Audio preview:** `rodio` 0.23.0 (MIT/Apache) on top of `cpal` 0.18.2 (Apache-2.0). A sine or ocarina-like tone made from oscillators is a few lines of code. `rustysynth` (MIT) can play a SoundFont if you ever want "real instrument" previews.
- **Drive detection:** `sysinfo` 0.40.4 (MIT) lists mounted disks with mount points. Check for `INFO_UF2.TXT` there, which is the same logic ADR-005 already describes. `std::fs::copy` copies the file. Watching for the drive to vanish means polling the disk list.
- **Piano roll and gamepad:** immediate-mode `Painter` (rects, lines, text, clip) suits a scrolling timeline with thousands of notes. The gamepad buttons can react to pointer press/release and key events that carry a `repeat` flag, so autorepeat cannot cause repeated presses.
- **Threads:** a worker thread owns the port and talks to the UI over `std::sync::mpsc`/`crossbeam` channels. The worker calls `ctx.request_repaint()` (thread-safe, checked in `context.rs`). The UI never blocks.
- **Native dialogs and HiDPI:** use `rfd` 0.18.0 (MIT) for open/save dialogs. HiDPI is handled by winit (`pixels_per_point`). Accessibility comes through AccessKit.
- **Distribution:** one executable. macOS needs a `.app` bundle (cargo-packager / cargo-bundle or a small script). The Rust linker already ad-hoc signs arm64 binaries, but that is not notarisation. Linux options are AppImage or a plain binary. Windows can ship a portable `.exe` or an MSI/NSIS installer.
- **Testability and CI:** put the core in a separate crate (`app-core`: score import, pipeline, COBS/CRC protocol, flasher logic) with no egui dependency. Test it with `cargo test` and fuzz COBS with `proptest`. CI is one matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`), and you can build x86_64 + aarch64 on macOS and merge them with `lipo`.
- **Weak spots:** non-native look, no wizard widget, 0.x API churn, and steeper error messages for a Rust newcomer.

### Python + PySide6 (runner-up)

- **Health (checked):** PySide6 6.12.0 came out on 2026-10-08 and 6.11.3 on 2026-10-26. Licence is LGPL-3.0 / GPL-2/3 / commercial.
- **Coverage is the best of all candidates:** `QSerialPortInfo` has `vendorIdentifier()`, `productIdentifier()`, `description()` and `serialNumber()`. `QStorageInfo.mountedVolumes()` gives volume names and root paths, so `RPI-RP2` detection needs no extra dependency. Other pieces: `QWizard` for the flashing flow, `QGraphicsView` or `QPainter` for the piano roll, `QKeyEvent.isAutoRepeat()` for the gamepad, native dialogs and HiDPI. All checked in the Qt for Python docs. Signals and slots make the worker-thread pattern idiomatic.
- **Distribution (measured):** a 113 MB onedir `.app` / 42 MB zipped for Widgets + SerialPort + Multimedia. QtMultimedia pulled in `libavcodec`, `libavformat` and `libavutil` (FFmpeg, LGPL). Using `sounddevice` (MIT) instead of QtMultimedia for the preview avoids FFmpeg and some size.
- **Licence:** onedir keeps Qt replaceable (LGPL relinking). You still have to ship the LGPL text and offer Qt's source or a link to it. This needs the NFR-07 relaxation already flagged in R12.
- **Packaging pain:** PyInstaller cannot cross-compile, so you need one runner per OS (same as Rust). Onefile mode unpacks to a temp folder at every start and is the mode most often flagged by antivirus (see §5). Linux builds must run on an old glibc base.
- **Library risks (checked):** `pyserial` 3.5 is from **Nov 2020**. Upstream commits came back in Feb 2026 but there has been no release. It works, but it is unmaintained in practice. `python-rtmidi` 1.5.8 (Nov 2023) has **wheels only up to CPython 3.12**. On 3.13+ it must be built from source (C++ toolchain + meson in CI). That matters only for live MIDI after v1, but it pins the Python version. `mido` 1.3.3 is from Oct 2024 (MIT, fine).
- **Strengths:** fastest UI iteration, Qt Designer if wanted, a huge amount of documentation, and the owner's existing plan.

### Rust + Slint

- **Health (checked):** 1.19.0 came out on 2026-11-16, with releases every ~2 months.
- **Licence (checked):** `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`. The royalty-free licence covers desktop apps if you either show the `AboutSlint` widget in an About screen or display the Slint badge on the download page. It excludes embedded systems and redistributing Slint on its own. An MIT-licensed app can use it, but Slint itself is not MIT. NFR-07 would need a note such as "Slint under its royalty-free licence, attribution shown".
- **Fit:** the declarative `.slint` markup with live preview is pleasant, and wizard screens are easy. The piano roll is clunkier: there is no immediate painter, so you use `Path` elements, many rectangles in a `for` loop, or render into a pixel buffer yourself. There is no built-in file dialog, so you use `rfd` (estimate based on docs reading).
- Choose Slint over egui if a polished look matters more than a simple canvas.

### Rust + Tauri 2

- **Health (checked):** 2.12.7 came out on 2026-11-18, 3.0.0-alpha.6 is in progress, and adoption is very large. Licence is MIT/Apache.
- **Fit:** the Rust backend handles serial, MIDI and drives with the same crates as egui. The front end is HTML/TS, where a piano roll on `<canvas>` and a nice wizard are easy. The output is a small binary because it uses the system WebView.
- **Costs:** two languages, an IPC boundary and a JS build chain. On Linux, WebKitGTK causes rendering problems on some NVIDIA setups, which Tauri's own docs acknowledge, and AppImages with bundled WebKitGTK break across distros/glibc. Windows relies on WebView2, which is present on Windows 10/11.
- Choose Tauri if the owner already knows web front-end work. Otherwise it adds stack for no gain.

### Rust + Iced

- **Health (checked):** 0.14.0 came out on 2025-12-07 and 0.13 on 2024-09, so a release roughly every 9–15 months. Master is very active (vello_cpu renderer work, Nov 2026). Licence is MIT.
- The Elm architecture is clean but takes more boilerplate and is harder for a newcomer than egui. `Canvas` handles the piano roll. Dialogs come from `rfd`. Long gaps between releases mean you either use git master or a year-old API.

### Rust + Dioxus desktop

- **Health (checked):** 0.7.10 (Jul 2026) is stable. 0.8 has been in alpha since May 2026. It uses a webview (wry) like Tauri, but with a React-like Rust DSL.
- It has the same Linux WebKitGTK caveats as Tauri and the API is still churning. Not yet.

### Rust + gtk4-rs / Rust + Qt (cxx-qt)

- gtk4-rs 0.11.5 is MIT, but GTK itself is LGPL. Shipping GTK on Windows/macOS means bundling a large runtime and dealing with fiddly theming. No.
- cxx-qt 0.10.0 (KDAB, MIT/Apache) needs a Qt C++ install, CMake/QML and LGPL Qt. It combines the worst learning curve with the licence problem. No.

### Python + CustomTkinter (current fallback in ADR-008)

- **Health and licence (checked):** 6.0.0 came out in Jun 2026 after a 2.5-year gap, with commits in Nov 2026. The LICENSE file says MIT, but the PyPI metadata field says CC0 while the classifier says MIT. Both are permissive, though the metadata is untidy.
- **Coverage gaps:** no native canvas beyond Tk `Canvas` (fine for a piano roll, slower with many items), no audio, and no drive detection (use psutil). On X11, Tk reports key autorepeat as **KeyRelease/KeyPress pairs**, so a hold-to-press gamepad needs debounce timers. HiDPI scaling and macOS look-and-feel have quirks.
- **Measured size:** 41 MB onedir / 19 MB zipped with numpy and sounddevice. It still has the PyInstaller antivirus profile.
- It fits NFR-07 literally, but it is the weakest end-user experience among the serious options.

### Python + Dear PyGui

- **Checked:** 2.3.1 (May 2026), MIT, wheels up to CPython 3.14, commits in Nov 2026.
- Fast plotting and drawing make the timeline easy, but file dialogs are custom (non-native) and it feels like a tool, not an app. That works against NFR-10 for first-time users.

### Python + Flet

- **Checked:** 1.1.2 (Nov 2026), Apache-2.0, with fast releases.
- `flet build` needs the Flutter SDK in CI and packages a Python runtime inside a Flutter app. Native wheels (pyserial is pure Python, but rtmidi and sounddevice are not) and drive access are less proven there. The bundle is large. Too many moving parts.

### Python + Toga (BeeWare)

- **Checked:** 0.5.7 (Oct 2026), BSD-3. Briefcase 0.4.7 packages it properly (better than PyInstaller).
- Custom drawing (`Canvas`) is limited and there is no audio. Native widgets differ per OS, so behaviour differs too. Still pre-1.0.

### Go + Fyne / Wails (outsider)

- Fyne (BSD-3) gives a single binary, and `go.bug.st/serial` (BSD-3) does USB VID/PID enumeration. Other pieces: `gitlab.com/gomidi/midi` (MIT) and `oto` (Apache) for audio. Wails v3 has very recently reached or is about to reach stable, and the sources disagree.
- Go is easier to learn than Rust, and its licences are clean. But Fyne's canvas and look are basic, and CGO means one runner per OS anyway. It is a credible alternative if Rust's learning curve is the blocker, but not clearly better than PySide6 for this owner.

### Electron (reference only)

- Expect a 80–150 MB bundle (estimate). `serialport` npm is MIT with a native module. Everything is easy to build and heavy to ship. It does not fit "small, nothing to install" better than PySide6.

---

## 4. Requirement-by-requirement coverage (top two)

| Need | Rust + egui | Python + PySide6 |
|---|---|---|
| Find bridge by VID/PID/product (FR-08) | `serialport::available_ports()` → `UsbPortInfo` | `QSerialPortInfo` or `pyserial.tools.list_ports` |
| Background serial worker | thread + channels + `request_repaint()` | `QThread` + signals |
| 1200-baud touch | open at 1200, close | same |
| `.mid` import, track/channel (FR-01) | `midly` | `mido` |
| Text score (FR-02) | hand-written parser (or `nom`/`winnow`) | hand-written parser |
| Piano roll (FR-12) | `Painter` | `QGraphicsView`/`QPainter` |
| Audio preview (FR-12) | `rodio`/`cpal` | `sounddevice` (MIT) or QtMultimedia (+FFmpeg LGPL) |
| On-screen gamepad, low latency (FR-11) | key events with repeat flag, pointer press/release | `isAutoRepeat()`, mouse press/release |
| `RPI-RP2` detection + UF2 copy (FR-09) | `sysinfo` disks + `std::fs` | `QStorageInfo` + `shutil` |
| Wizard (NFR-10) | hand-written state machine | `QWizard` |
| Live MIDI (FR-13, after v1) | `midir` | `python-rtmidi` (wheels ≤ 3.12) |
| Licence (NFR-07) | permissive + MPL-2.0 serialport | LGPL Qt (+ LGPL FFmpeg if QtMultimedia) |

---

## 5. Cross-cutting: signing, Gatekeeper, SmartScreen, antivirus (R5)

Signing is independent of the stack. Changing language does not resolve R5, it only changes how bad things are while unsigned.

- **macOS:** since **Sequoia (15)**, Control-click → Open no longer bypasses Gatekeeper for unsigned or un-notarised apps. The user has to try to launch, then go to System Settings → Privacy & Security → "Open Anyway". **The R5 mitigation text ("right-click → Open") is out of date.** For NFR-10 with first-time users, budget for the Apple Developer Program (≈ 99 USD/yr) and notarisation in CI (`notarytool`). The cost and effort are the same for Rust and Python, but notarising PyInstaller bundles needs each nested dylib signed with the hardened runtime, which is more fiddly than a single Rust binary.
- **Windows:** SmartScreen warns about any new unsigned executable until it builds reputation, whatever the language. On top of that, PyInstaller bootloaders, especially onefile and UPX-compressed builds, regularly trigger ML-heuristic detections (e.g. Defender `…!ml`). Mitigations are to drop UPX, embed version info, report false positives to vendors and sign. Free OSS signing is available from the **SignPath Foundation**: OSI licence, fully automated build, MFA, and the certificate is issued to SignPath Foundation. **Azure Artifact Signing** (formerly Trusted Signing) accepted individuals only in the US/Canada during preview, as of a Mar 2026 Microsoft Q&A answer. Check eligibility before relying on it.
- **Nuitka is not a clean way out for Python:** it **changed its licence to AGPLv3** (with a runtime exception for compiled programs) in Jan 2026 (checked in repo history). Using it for builds is allowed, but it is a licence review item and does not change the signing story.
- **Linux:** no signing gate. Remaining issues are the `dialout` group and udisks auto-mount, which are the same for every stack.

---

## 6. Risks

| # | Risk | Applies to | Mitigation |
|---|---|---|---|
| S1 | Rust learning curve delays M2–M6 | Rust | time-boxed spike with exit criteria (§1); keep core algorithms simple; AI-assisted pairing |
| S2 | egui 0.x API breaks | egui | pin versions; upgrade deliberately once per milestone |
| S3 | `serialport` MPL-2.0 vs NFR-07 text | Rust | amend NFR-07 to "MIT/BSD/Apache/MPL-2.0 (unmodified)" or use `serial2` + `nusb` |
| S4 | LGPL compliance (Qt, FFmpeg) | PySide6 | onedir bundle, licence files, avoid QtMultimedia |
| S5 | PyInstaller antivirus false positives | all Python | onedir, no UPX, version info, signing, vendor reports |
| S6 | pyserial unreleased since 2020; python-rtmidi wheels stop at 3.12 | Python | pin Python 3.12 (security support to Oct 2028); vendor or build rtmidi in CI |
| S7 | macOS Gatekeeper without notarisation breaks NFR-10 | all | Apple Developer account + notarisation in CI; fix R5 text |
| S8 | Linux WebKitGTK rendering/glibc issues | Tauri, Dioxus | prefer a non-webview toolkit |
| S9 | `RPI-RP2` auto-mount on Linux | all | detect unmounted device and explain (ADR-005), or PICOBOOT via `nusb`/`pyusb` |

---

## 7. Decision options for the owner

1. **Reopen ADR-005 → Rust + egui** (recommended, after the spike passes). This also settles ADR-008 (egui) and R12 (no LGPL), and NFR-07 only gains MPL-2.0.
2. **Keep ADR-005 → accept ADR-008 with PySide6** and relax NFR-07 to "permissive or LGPL with dynamic linking". This is the fastest path to a rich UI and the heaviest to ship.
3. **Keep ADR-005 with CustomTkinter** to keep NFR-07 literal. Not recommended: the most hand-written UI work, the weakest UX, and the PyInstaller downsides remain.

Whatever you choose, R5 needs its own decision. Signing and notarisation are the real gate for NFR-10 on macOS and Windows.

---

## 8. Sources

Registries and repositories (versions, dates, licences, all checked 2026-11-19):
- egui / eframe: https://crates.io/crates/egui, https://crates.io/crates/eframe, https://github.com/emilk/egui
- egui `request_repaint` from other threads: https://github.com/emilk/egui/blob/main/crates/egui/src/context.rs
- egui_plot: https://crates.io/crates/egui_plot
- Iced: https://crates.io/crates/iced, https://github.com/iced-rs/iced/commits/master
- Slint: https://crates.io/crates/slint; licence: https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md
- Tauri: https://crates.io/crates/tauri; Linux graphics issues: https://v2.tauri.app/develop/debug/linux-graphics/
- Tauri AppImage/WebKitGTK reports: https://www.github.com/firezone/firezone/issues/4883, https://dev.to/fengzeng/fixing-wayland-crashes-in-tauri-appimage-linuxdeploy-gtk-issue-1381
- Dioxus: https://crates.io/crates/dioxus
- gtk4-rs: https://crates.io/crates/gtk4; cxx-qt: https://crates.io/crates/cxx-qt
- serialport (MPL-2.0): https://crates.io/crates/serialport, https://github.com/serialport/serialport-rs
- serial2: https://crates.io/crates/serial2, https://docs.rs/serial2/latest/serial2/struct.SerialPort.html
- nusb: https://crates.io/crates/nusb; midly: https://crates.io/crates/midly; midir: https://crates.io/crates/midir
- rodio: https://crates.io/crates/rodio; cpal: https://crates.io/crates/cpal; rustysynth: https://crates.io/crates/rustysynth
- sysinfo: https://crates.io/crates/sysinfo; rfd: https://crates.io/crates/rfd
- PySide6: https://pypi.org/project/PySide6/; QSerialPortInfo: https://doc.qt.io/qtforpython-6/PySide6/QtSerialPort/QSerialPortInfo.html; QStorageInfo: https://doc.qt.io/qtforpython-6/PySide6/QtCore/QStorageInfo.html
- CustomTkinter: https://pypi.org/project/customtkinter/, https://github.com/TomSchimansky/CustomTkinter
- Dear PyGui: https://pypi.org/project/dearpygui/; Flet: https://pypi.org/project/flet/; Toga: https://pypi.org/project/toga/; Briefcase: https://pypi.org/project/briefcase/
- pyserial: https://pypi.org/project/pyserial/, https://github.com/pyserial/pyserial; distro status: https://bugzilla.redhat.com/show_bug.cgi?id=2492961
- python-rtmidi: https://pypi.org/project/python-rtmidi/; mido: https://pypi.org/project/mido/; sounddevice: https://pypi.org/project/sounddevice/; psutil: https://pypi.org/project/psutil/
- PyInstaller: https://pypi.org/project/pyinstaller/
- Nuitka licence (AGPLv3 + runtime exception, changed 2026-01-28): https://github.com/Nuitka/Nuitka/blob/main/README.rst, https://pypi.org/project/Nuitka/
- Wails releases: https://github.com/wailsapp/wails/releases, https://v3.wails.io/blog/

Signing, Gatekeeper, antivirus:
- Sequoia removes the Control-click Gatekeeper override: https://mjtsai.com/blog/2024/07/05/sequoia-removes-gatekeeper-contextual-menu-override
- SignPath Foundation (free OSS code signing): https://signpath.org/, https://signpath.org/terms
- Azure Artifact Signing pricing: https://azure.microsoft.com/pricing/details/artifact-signing/; individual eligibility thread: https://learn.microsoft.com/en-us/answers/questions/5810735/cant-create-a-new-trusted-signing-individual-ident
- PyInstaller false positives: https://github.com/microsoft/apm/issues/487, https://github.com/pyinstaller/pyinstaller/issues/5579, https://discuss.python.org/t/pyinstaller-false-positive/43171

Measurements (this report): PyInstaller 6.22, CPython 3.12, macOS arm64, `--windowed` onedir. Size is from `du -sh`; zipped size is from `zip -ry -9`. Rust binary sizes were not measured (no toolchain available) and are marked as estimates.
