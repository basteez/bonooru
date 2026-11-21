# Comparable projects: UX research

Date: 2026-11-20. Read-only research of two open-source "PC app + microcontroller as Switch controller" tools.
Sources were read via `gh` and raw.githubusercontent.com. Screenshots were downloaded and viewed (3 Friend Maker UI shots).
Short refs used below: **TD** = `xoaninc/TomodachiDrawer-V2` (fork of `Lucas7yoshi/TomodachiDrawer`), **FM** = `zhouxiyu1997/friendmaker`.

## 0. The key difference between the two

| | TomodachiDrawer V2 | Friend Maker |
|---|---|---|
| UI stack | Avalonia (C#/.NET) desktop, one window (`TomodachiDrawer.V2/MainWindow.axaml`) | Electron wrapping a local web UI (`apps/desktop/src/web/static/index.html`, `app.js`, `i18n.js` zh/en) |
| Board | RP2040 / RP2350 / ESP32-S3, **USB wired** to Switch | ESP32-WROOM-32, **Bluetooth Classic** to Switch, USB serial to PC |
| Execution model | **Offline**: the drawing is flashed onto the board, the board is unplugged from the PC and plugged into the Switch, and it replays on its own. The PC has no live link. | **Live**: the PC streams commands over serial with ACKs while drawing, so it can pause, resume and show progress. |
| Closest to bonooru | Setup and flashing copy, in-game preconditions | Live connection status, pause/stop/resume, readiness gating. **This is the closer analogue** to our bridge+controller live link. |

## 1. TomodachiDrawer V2

### Setup / onboarding
- **Flashing is done in the app.** The app bundles `.uf2` and `.bin` firmware files (`TomodachiDrawer.V2/TomodachiDrawer.Firmware.*`). For RP2040 the button is "Flash Base Firmware", which copies the file to the BOOTSEL drive. For ESP32 the app has its own flasher (`EspFlasher.cs`) that verifies the write by MD5 and flashes again if it does not match (README, "What's new").
- **Two-step mental model, explained in a dialog:** "You only need to flash the code/"firmware" once. You then flash the image data onto it for each image" (`MainWindow.axaml.cs:1768-1779`, Help → "Setup Explanation (Pico)").
- **Manual fallback is documented:** "If it does not detect it, you can still drag-and-drop the .uf2 … select 'export .uf2'" (README, "If the program does not recognize your RP2040").
- **Switch setting:** the copy shouts it. "YOU MUST HAVE "Pro Controller Wired Commmunication" ENABLED. Go to system settings -> …" (`MainWindow.axaml.cs:1778`). It is repeated in the README, in the post-export dialog and in the ESP32 guide. Despite this, it is still the first reply in upstream issue #141.
- **First run:** a welcome dialog with a hand-incremented ID (`CURRENT_WELCOME_ID = 3`) is shown again whenever something notable changes. Help → "Show Welcome Again" re-opens it (`MainWindow.axaml.cs:199-212`, `.axaml:100`). On Windows there is also a check for the VC++ runtime that shows "WARNING: MISSING LIBRARIES".
- **Video-first onboarding:** "Following the YouTube tutorial is recommended", plus an addendum video (README). The text steps are numbered, and step 2 is missing from the list (README, "Or briefly, in text").
- **Platform permission copy for macOS:** "Permission to access the {drive} was denied… open System Settings -> Privacy & Security -> Files & Folders … 'Removable Volumes'". It also offers a manual-copy alternative (`.axaml.cs:360-363`).

### Device connection & status
- Device detection is automatic, and the result is shown as a plain label per chip tab: "RP2040: Not Found", "ESP32 Status: Not Found" (`MainWindow.axaml:298,379`).
- **The status text says how to fix the problem:** "ESP32 not found — hold BOOT, tap RESET, release BOOT" and "ESP32: {n} serial port(s) — pick the one in download mode" (`.axaml.cs:566,577`). The app also lists ports it ignored and why: "Ignored, because these cannot be a board:".
- There is a manual COM-port dropdown for ESP32 only. The RP2040 is found automatically by its drive.
- There is no live link to the Switch, so there is no "connected to Switch" indicator. **The board's LED is the status display:** "dim white = idle, blinking yellow = startup countdown, green = drawing, red blink = no/invalid image data, rainbow = finished" (README, ESP32 section). The dialog also says "Flashing red is expected" after the first firmware flash (`.axaml.cs:1775`). Detection is fragile in practice: issue #156 reports "always shows 'RP2040: Not Found'" and was fixed by re-entering BOOT mode.

### Main screen IA
- **One dense window:** a menu bar (File / Settings / Templates / Tools / Help). The left column has Input Image, Routing options, Log, the time estimate and a per-chip **tab strip** for output. The right side has the Preview with colour matcher, limit and denoise (`MainWindow.axaml`).
- **Primary actions are the export buttons with "!":** "Export To RP2040!", "Export To ESP32!". There is also a secondary "Estimate" button that computes the route without flashing.
- **Time estimate:** "Draw Time Estimate: ???", then "estimating…", then a value. The value is suffixed "(nominal; HID timing varies)" when experimental timing is on (`.axaml:277`, `.axaml.cs:1414,1534`).
- **No run progress** while drawing, because the board runs offline. "Cancel Generation" has an honest tooltip: "Only affects generation on this computer — it cannot stop a drawing already running on the Switch" (`.axaml:239`). The way to stop a drawing is to unplug the board (`Docs/HARDWARE_VALIDATION_0.6.0.md`).
- **The log is a first-class panel** and is also saved to a file ("Log also saved to: {path}").
- Every option has a `?` button or a tooltip. The TSP help text is long and technical (`.axaml.cs:1045-1051`).

### Virtual gamepad / manual control
- `TomodachiDrawer.DebugTools/UI/VirtualGamepadControllerWindow.axaml` is a full on-screen pad (ZL/L/ZR/R, sticks, D-pad, ABXY). It lives in the **debug tools** project and is used to drive the simulator, not the real console.
- `Docs/POST_0.5.0_IDEAS.md` lists RiiDraw's "**Virtual on-screen gamepad** — manual nudging during setup, without unplugging the board" as a real gap. The same doc credits RiiDraw (closed source, **two RP2040 boards**, one facing the PC and one emulating the Pro Controller, like bonooru) with "automatic cursor calibration".

### Unknown in-game state
- The app assumes the start state and documents it as a **precondition checklist**. Help → "In-Game Setup Info": "Navigate to the palette house / Ensure you are on the "advanced" drawing UI / top colour … Black / cursor to the TOP LEFT / … zoomed out" (`.axaml.cs:1783-1794`). The rationale is explained: "If the canvas is zoomed in, it will cause the cursor to desync…".
- The **"Auto Home Canvas To Top Left"** checkbox moves the cursor into a known state by pushing it against the clamps ("homing slams into clamps", `Docs/FIRST_CONNECTION_OFFSET.md`). It is turned on automatically for full-size images, with a log line explaining why (`.axaml.cs:838`).
- **The post-action dialog restates the next physical steps:** "Drawing data flashed … Now RESET to start it (this is the moment to reset): 1. Unplug … 2. On the Switch: enable … open Palette House…" (`.axaml.cs:778-784`).

### Errors, recovery, pitfalls (issues on upstream `Lucas7yoshi/TomodachiDrawer`)
- **Desync is the dominant complaint.** Long drawings stop or drift: #53 "Stopped before finishing drawing", #115 "stop if around one and a half hour", #145 "Works half way then suddenly starts losing it", #66, #33. In #207, changing the monitor's HDMI input mid-run desyncs the drawing; the owner kept it open as a warning to other users.
- There is **no resume:** "A 256² draw is ~2 hours. A desync or an interruption means starting over" (`POST_0.5.0_IDEAS.md`, "Resume from a checkpoint"). The proposed after-the-fact fix is "Diff-image repair" (#159).
- Requested features: "cancel button" (#162, added), "calculate drawing time even if device is not plugged" (#110). Several issues report crashes or the app not opening (#136, #135, #92, #93). #133 reports the window could not be resized and the bottom was unreachable, so the layout did not fit small screens.
- Error dialogs often have retry built in: "Close anything that might be using the drive, then click OK to retry" (`.axaml.cs:1404`). Some are informal: "This REALLY shouldn't happen... Try reinstalling?", "maybe try doing it manually..?".

### Tone
Friendly and informal, sometimes apologetic ("Note: flashing lights warning for video, apologies"). It uses ALL-CAPS for critical prerequisites. The README is honest about limits: the "Hardware validation" section says desync has been reported on both generations and "Offline rendering … cannot establish that the game received every input".

## 2. Friend Maker

### Setup / onboarding
- **The app is organised as a four-step pipeline, shown as big top tabs.** Each tab has a title and a sub-line (screenshots `docs/media/ui-*.jpg`): "Script Studio — Import, preview, execute", "Firmware Flash — compile, burn, troubleshoot", "Controller Test — Connection, status, single-step actions", "Timing Tune — Timing, test presses, square/dot-matrix". The docs order is Flash → Controller Test → Timing → Studio (`docs/en/user-trial-guide.md`), but the tab bar puts Studio first.
- **Flashing is done in the app through PlatformIO.** If PlatformIO is missing, the app offers a "Prepare PlatformIO" button and downloads an app-local Python. There is a one-click Windows CP210x/CH340 driver installer, which bundles the drivers in `drivers/windows/`. There is also a fallback web flasher on GitHub Pages. You pick the Switch model first ("Switch1 and Lite firmware" / "Switch 2") (README, guide §4).
- **Expectations are set up front:** "This is not a `zero-setup`, consumer-style plug-and-play tool… leave time for setup and debugging" (README "Before you start"). This is repeated in the guide.
- **Switch-side pairing step:** "On Switch, open `Controllers -> Change Grip/Order`" (guide §5).
- **Wiring:** a single USB cable; the docs repeat "a USB cable that supports data transfer" (`docs/en/wiring.md`).
- **First-run "safe mode" recipe:** "For your first successful run, prefer: mono drawing, brush size 3, … something structurally simple and high-contrast" (guide §7.1).

### Device connection & status
- **Ports are picked manually** from a dropdown that shows the path and the chip vendor ("/dev/cu.usbserial-0001 Silicon La…"). Next to it are "Refresh ports" and "Disconnect Serial". The status line reads "Serial port in use: /dev/cu.usbserial-0001 @ 115200" (controller-page screenshot). Empty state: "No serial ports detected".
- **The connection status is broken down into stages.** A "Connection Status" panel has a big state pill and headline ("Ready" / "Controller connected"). Under it are tiles for **Discoverable / Authenticated / Connected / Paired / Ready to send**. Below that are technical rows: Transport, Profile, Last host MAC (controller screenshot; strings in `i18n.js`). States include "Pending connection", "Connection established", "Connection may drop", "Initialization error", "Disconnected".
- **Readiness is shown across pages and gates drawing.** Script Studio shows a green card "Controller is connected. Drawing can start." with a "Go to Controller Test" button. When not ready it says: "The board has not been confirmed as connected to the Switch yet. Before drawing, finish the connection on the Controller Test page."
- **There is automatic recovery on timeout:** "Connection has waited over 45 seconds. Automatically resetting Bluetooth and retrying once." If that fails: "Auto-recovery did not complete. Click Connect Controller again; if it still gets stuck, press the board's EN button and retry."
- Port errors: "The current serial port is unavailable or the port name changed. Refresh the port list, select the target again, and retry." / "Previously selected serial port disconnected".

### Main screen IA
- Tabs over **two-column cards**. Left column: controls. Right column: status and preview. The Studio page shows a preview with a grid and big stat tiles: Canvas range, Image size, Position, Colours "31 / 32 auto", Pixels, Commands, **Estimated time "258m 4s"** (studio screenshot).
- **Two-stage start:** "Generate Commands Only" (inspect the preview and stats) → "Start Drawing" (guide §7.2).
- **Run controls:** "Pause Drawing" / "Resume Drawing" / "Stop and Save Resume Point", with the caveat "Pause and stop take effect after the current command finishes." There is also a separate "Emergency action: Use only if stopping stays unresponsive for a long time", which has a confirm dialog explaining that it force-clears the run state.
- Run states: "In progress", "Paused", "Stopping", "Resume point saved", "Interrupted by error", "Failed", "Completed", "Retry needed".
- **Result panel after a run:** "Shows elapsed time, theoretical time, and device feedback": measured total time, theoretical time, average per action, and the last device reply (`index.html:1214-1240`).

### Virtual gamepad / manual control
- The Controller Test page has an on-screen **stick pad, D-pad and button grid** (L, ZL, X, Y, …), plus a "stick steps" selector ("1 cell"). These send single-step commands to the real Switch: "This page uses single-step commands to test the real device, without relying on a full image script." The screen also shows the active timing ("CFG INPUT 65 45 1800").
- **The timing page uses a "recommended first" layout.** On the left: "Tune these two knobs first", with plain-language names plus the technical name ("Stability wait — technical name: inputDelay", "Suggested start 45ms", "Common range" badge). On the right: three test cards in increasing severity, each with a duration and action count ("Quick Check… ~9.3 s · 85 actions", "Drift check ~34 s", "Real long-run ~5.5 min · 2998 actions") (timing screenshot).

### Unknown in-game state
- **The app assumes a fixed start state and asks for it every time with a confirm dialog before any run:** "Before starting, confirm that the Switch is on the drawing page, the brush tool is active, preferably brush 1 is selected, the brush is at the canvas center, and you will not touch the controller or screen from now on. Run Quick Check … now?" (`i18n.js`).
- **The start state is chosen to be easy to re-establish.** Canvas centre is used "rather than top-left", because the canvas cannot be reliably homed ("does not treat the main canvas as a bounded plane that can stably 'return to top-left'", README limits).
- **Resume confirmation:** "Confirm that you have saved the current artwork on the Switch and manually re-entered the drawing page. From here on, do not manually change the brush or move the page. Resume from the saved point now?"
- The "Three things you must know before drawing" list in the guide (§1.4) states the preconditions explicitly.
- The app removes one assumption itself: "the app switches to the selected pixel brush automatically, so you no longer need to choose it manually in-game".

### Errors, recovery, pitfalls
- **Recovery sessions** are saved locally on pause, stop, crash or restart. They are segment-based (`apps/desktop/src/app/recovery.ts`, `ResumeProgressSnapshot`), and the tasks are listed in-page under "Resume tasks".
- **Troubleshooting is organised by page:** "Use it by asking one question first: which page are you blocked on right now?" (`docs/en/troubleshooting.md`). It gives numbered escalation ladders (reset BT → reconnect → re-open Change Grip/Order → press EN → reflash).
- **Issues:** Bluetooth pairing is unstable across boards (#36, #26, #76). There are PlatformIO and Python toolchain flashing failures (#56, #83). #82 "frequently shows drawing interrupted by error" was closed by the owner for lack of information, so the app does not help users file useful reports. A Linux serial lag issue corrupted the ACK stream (#90). The owner's support replies are long numbered recipes in both Chinese and English (#41).
- The app is honest about hardware variance: "Some ESP32 compatible boards still vary…", "some boards become less stable when they get hot".

### Tone
Calm, explanatory, and bilingual (a zh/en toggle via i18next). Expectations are set explicitly ("not zero-setup", "stability not speed"). It keeps technical names visible next to the friendly labels. The visual design follows `DESIGN.md`: "Warm cream desktop tool with Animal Crossing illustration accents", with a coral active tab and pill badges such as "Recommended first", "Step 2", "Final repro".

## 3. Cross-cutting observations
- Both apps **cannot see game state** and handle it the same way: **a documented precondition checklist, plus restating it at the moment of action**. FM uses a modal confirm before every run. TD uses a Help dialog and the post-export dialog. Neither tries to detect the game state.
- Both pick a start state that can be **re-established by brute force**. TD homes the cursor into the canvas corner by pushing against the clamps. FM uses the canvas centre, which the game resets on page entry.
- Failure modes are physical and silent (desync, HDMI input switch, a touched controller). The best defence seen is **telling users what not to do during a run** ("do not touch the controller or the screen", "Do not unplug the board").
- Long runs without resume are the most-complained-about cost (TD). FM's resume point is its strongest UX feature.
- Status text works best when it **names the fix**: TD's "ESP32 not found — hold BOOT, tap RESET, release BOOT", and FM's connection-timeout message.

## Transferable to bonooru (candidates, not decisions)
Context: Rust + egui desktop app, two RP2040 boards (bridge on the PC side, controller on the Switch side, like RiiDraw), live link, playing melodies in OoT free play. Game state cannot be detected. No cursor calibration.

1. **Pre-play confirm checklist, short and specific.** This adapts FM's modal: "Link has the ocarina out (free-play), controller not touched from now on". It could be a checklist modal or an inline checkbox the first time, then an optional "don't ask again". The steps could be restated in one line next to the Play button.
2. **Staged connection status with the fix in the text.** A pill/tiles strip PC↔Bridge, Bridge↔Controller, Controller↔Switch, Ready, with each failed state naming the action ("Controller board not detected — check the cable between the two boards"). Play is gated on "Ready", with a "Go to connection" link (FM Studio card).
3. **Run controls that are honest about timing.** Play / Stop, with "stops after the current note" semantics stated in the copy. Stop is the emergency action (release all buttons). The post-run summary could show the measured vs expected duration.
4. **Manual control / test pad.** A small on-screen pad for the ocarina buttons (A, C-buttons, R/Z for pitch, stick for bend) plus single-note "test press" buttons. These verify the link and the in-game state without unplugging. RiiDraw has this and the TD maintainers list it as a gap. FM's controller-test page shows how.
5. **First-run safe recipe plus a test melody.** FM recommends "simplest settings first" and has graded test cards (quick → long, each with a duration). For us this could be "Play test: Zelda's Lullaby (8 s)" before custom songs. A timing page could use plain-language knob names with the technical name shown beside them, and a suggested default.
6. **Onboarding as a linear set of steps the user can come back to.** Flash firmware → Connect → Test → Play, as tabs or a first-run wizard. Each step should say what physical action is next (TD's "this is the moment to reset"). Add a "Show welcome again" entry and a welcome dialog versioned by ID.
7. **Firmware flashing inside the app, with a fallback.** Bundle the UF2 files, detect BOOTSEL drives, verify the write, and also document drag-and-drop of the UF2 as a fallback (TD). Show the firmware version and the expected LED pattern after each step ("blinking X is expected").
8. **Persistent log panel plus a log file, and "copy diagnostics".** Both projects lean on logs, and FM #82 shows that users report "error" with no data. A one-click "copy report" (versions, ports, last N log lines) is a candidate.
9. **Troubleshooting docs organised by "where are you stuck"**, with numbered escalation ladders (FM). The README should put Switch prerequisites in the first screen of text: TD's "Wired Pro Controller Communication" still causes issues despite ALL-CAPS copy. In-app copy could link straight to the exact Switch menu path.
10. **Pitfalls to avoid:** a dense single window that does not fit small screens (TD #133); dialogs that apologise without giving a next step; a tab order that does not match the documented flow (FM); hiding that a cancel only stops the PC side (TD's honest tooltip is the good counter-example).
