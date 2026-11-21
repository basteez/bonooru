# Architecture diagrams

Mermaid sources from the notes. "Host" in labels = the app. Link sequence, USB enumeration and score pipeline diagrams live in `docs/link-protocol.md`, `docs/usb-controller-emulation.md`, `docs/score-format-and-mapping.md`.

## System

```mermaid
flowchart TB
  subgraph PC["PC (Windows / Linux / macOS)"]
    IMP["Score import<br/>(.mid, text)"] --> ARR["Arranger<br/>mono + range fit"]
    ARR --> FNG["Fingering<br/>profile-driven"]
    FNG --> CMP["Timeline compiler<br/>µs events"]
    CMP --> STR["Streamer<br/>credit flow control"]
    PAD["Virtual gamepad"] --> STR
    FLS["Flasher<br/>UF2 / BOOTSEL"]
  end
  STR -- "USB-CDC" --> BR["RP2040-Zero #1<br/>Bridge"]
  BR -- "UART 3.3 V<br/>GP0/GP1 crossed + GND" --> CT["RP2040-Zero #2<br/>Controller"]
  CT -- "USB HID<br/>(wired controller)" --> SW["Switch 2<br/>Ocarina of Time"]
  FLS -. "BOOT + USB<br/>(one board at a time)" .-> BR
```

## Controller board internals

```mermaid
flowchart TB
  UART["UART RX (IRQ, break flag)"] --> DEC["Frame decoder<br/>COBS + CRC, type range, seq"]
  DEC --> MODE["Modes IDLE / DIRECT / PLAYING<br/>+ abort latch"]
  MODE -->|SET_STATE| RT["Real-time state"]
  MODE -->|QUEUE_EVENT| Q["Event queue<br/>(ring buffer)"]
  TMR["1 MHz timer<br/>playback clock"] --> SCH["Scheduler"]
  Q --> SCH
  SCH --> MASK["Safety mask<br/>allow-list in PLAYING"]
  MASK --> CUR["Published state<br/>(seqlock)"]
  RT --> CUR
  CUR --> PRF["HID profile<br/>build_report"]
  PRF --> USB["TinyUSB HID<br/>IN endpoint"]
  WD["Link / app watchdog,<br/>USB events"] -->|abort: NEUTRAL, set latch| MODE
```

## GUI screens (first idea, Rust + egui per ADR-008)

```mermaid
flowchart LR
  START["Start"] --> CHK{"Bridge found?<br/>Firmware OK?"}
  CHK -- no --> WIZ["Setup wizard<br/>BOOT + plug, flash bridge,<br/>flash controller, wiring photo"]
  WIZ --> CHK
  CHK -- yes --> SONG["Song<br/>open .mid / text, pick track,<br/>transpose, piano roll + preview"]
  SONG --> PLAY["Play<br/>countdown, progress, STOP"]
  CHK -- yes --> PAD["Gamepad<br/>reach free play"]
  PAD --> PLAY
  SONG -.-> SET["Settings<br/>profile, timing, advanced"]
```
