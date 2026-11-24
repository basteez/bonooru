//! Wire constants (AD-2, AD-14): the only place a wire number is written in
//! the app. Mirrored, value for value, from
//! `firmware/common/proto/include/ocb_proto/consts.h` and checked in both
//! directions against `firmware/common/proto/vectors/constants.txt` by
//! `tests/vectors.rs` (C.1). Names are the C macros without `OCB_`.

macro_rules! wire_consts {
    ($($name:ident : $ty:ty = $val:expr;)*) => {
        $(
            #[allow(missing_docs)]
            pub const $name: $ty = $val;
        )*
        /// Every constant above as `(name, value)`, for the conformance test.
        pub const ALL: &[(&str, u32)] = &[$((stringify!($name), $name as u32),)*];
    };
}

wire_consts! {
    // Protocol version (AD-7)
    PROTO_VERSION: u8 = 0x01;

    // Message types (AD-3)
    MSG_HELLO: u8 = 0x01;
    MSG_SET_STATE: u8 = 0x02;
    MSG_QUEUE_EVENT: u8 = 0x03;
    MSG_START: u8 = 0x04;
    MSG_STOP: u8 = 0x05;
    MSG_PING: u8 = 0x06;
    MSG_HEARTBEAT: u8 = 0x40;
    MSG_INFO: u8 = 0x81;
    MSG_STATUS: u8 = 0x83;
    MSG_ACK: u8 = 0x84;
    MSG_NAK: u8 = 0x85;
    MSG_EVENT: u8 = 0x86;

    // Type ranges per direction (AD-3)
    TYPE_APP_MIN: u8 = 0x01;
    TYPE_APP_MAX: u8 = 0x3f;
    TYPE_BRIDGE_MIN: u8 = 0x40;
    TYPE_BRIDGE_MAX: u8 = 0x7f;
    TYPE_CTRL_MIN: u8 = 0x80;
    TYPE_CTRL_MAX: u8 = 0xff;

    // NAK reasons (AD-6): transport 01..0f, semantic 10..7f
    NAK_CRC: u8 = 0x01;
    NAK_SEQ: u8 = 0x02;
    NAK_FULL: u8 = 0x03;
    NAK_VERSION: u8 = 0x10;
    NAK_BUSY: u8 = 0x11;
    NAK_ORDER: u8 = 0x12;
    NAK_BAD_STATE: u8 = 0x13;
    NAK_UNKNOWN_TYPE: u8 = 0x14;
    NAK_NO_LINK: u8 = 0x15;
    NAK_NOT_READY: u8 = 0x16;
    NAK_ABORTED: u8 = 0x17;
    NAK_TRANSPORT_MIN: u8 = 0x01;
    NAK_TRANSPORT_MAX: u8 = 0x0f;
    NAK_SEMANTIC_MIN: u8 = 0x10;
    NAK_SEMANTIC_MAX: u8 = 0x7f;

    // EVENT codes (AD-5, AD-8); NONE only in STATUS.last_event
    EVT_NONE: u8 = 0x00;
    EVT_PLAYBACK_DONE: u8 = 0x01;
    EVT_LINK_LOST: u8 = 0x02;
    EVT_LATE: u8 = 0x03;
    EVT_UNDERRUN: u8 = 0x04;
    EVT_USB_SUSPENDED: u8 = 0x05;

    // Controller modes (AD-8)
    MODE_IDLE: u8 = 0x00;
    MODE_DIRECT: u8 = 0x01;
    MODE_PLAYING: u8 = 0x02;

    // HID profile ids (INFO.hid_profile)
    HID_PROFILE_UNKNOWN: u8 = 0x00;
    HID_PROFILE_P1: u8 = 0x01;
    HID_PROFILE_P2: u8 = 0x02;
    HID_PROFILE_P3: u8 = 0x03;

    // Flag bits (AD-4, AD-8, AD-19)
    HB_FLAG_DTR: u8 = 0x01;
    STATUS_LINK_OK: u8 = 0x01;
    STATUS_CONSOLE_READY: u8 = 0x02;
    STATUS_RESERVED_MASK: u8 = 0xfc;
    STATE_FLAG_END: u8 = 0x01;
    STATE_RESERVED_MASK: u8 = 0xfe;

    // Buttons, canonical P1 bit order (AD-10)
    BTN_Y: u16 = 0x0001;
    BTN_B: u16 = 0x0002;
    BTN_A: u16 = 0x0004;
    BTN_X: u16 = 0x0008;
    BTN_L: u16 = 0x0010;
    BTN_R: u16 = 0x0020;
    BTN_ZL: u16 = 0x0040;
    BTN_ZR: u16 = 0x0080;
    BTN_MINUS: u16 = 0x0100;
    BTN_PLUS: u16 = 0x0200;
    BTN_L3: u16 = 0x0400;
    BTN_R3: u16 = 0x0800;
    BTN_HOME: u16 = 0x1000;
    BTN_CAPTURE: u16 = 0x2000;

    // Hat, clockwise from up; 8 = neutral (AD-10)
    HAT_UP: u8 = 0x00;
    HAT_UP_RIGHT: u8 = 0x01;
    HAT_RIGHT: u8 = 0x02;
    HAT_DOWN_RIGHT: u8 = 0x03;
    HAT_DOWN: u8 = 0x04;
    HAT_DOWN_LEFT: u8 = 0x05;
    HAT_LEFT: u8 = 0x06;
    HAT_UP_LEFT: u8 = 0x07;
    HAT_NEUTRAL: u8 = 0x08;
    STICK_CENTER: u8 = 0x80;

    // Sizes (D5): body <= 255, raw = 3 + body + 2, COBS adds ceil(n / 254)
    STATE_SIZE: usize = 0x08;
    MAX_BODY: usize = 0xff;
    MIN_RAW_FRAME: usize = 0x05;
    MAX_RAW_FRAME: usize = 0x0104;
    MAX_ENCODED_FRAME: usize = 0x0106;
    MAX_WIRE_FRAME: usize = 0x0107;
    FRAME_DELIMITER: u8 = 0x00;
    FRAME_HEADER_SIZE: usize = 0x03;
    FRAME_CRC_SIZE: usize = 0x02;

    // Body lengths (v1 sizes; D10 length rules)
    BODY_LEN_HELLO: usize = 0x01;
    BODY_LEN_INFO: usize = 0x0e;
    BODY_LEN_SET_STATE: usize = 0x08;
    BODY_LEN_QUEUE_EVENT: usize = 0x0c;
    BODY_LEN_START: usize = 0x04;
    BODY_LEN_STOP: usize = 0x00;
    BODY_LEN_PING: usize = 0x00;
    BODY_LEN_HEARTBEAT: usize = 0x04;
    BODY_LEN_STATUS: usize = 0x0b;
    BODY_LEN_ACK: usize = 0x01;
    BODY_LEN_NAK: usize = 0x03;
    BODY_LEN_EVENT: usize = 0x01;

    // CRC-16/CCITT-FALSE (RevEng CRC-16/IBM-3740), sent little-endian
    CRC16_POLY: u16 = 0x1021;
    CRC16_INIT: u16 = 0xffff;
    CRC16_CHECK: u16 = 0x29b1;

    // Link timing (D6); units in the name
    LINK_BAUD: u32 = 0x070800;
    HEARTBEAT_PERIOD_MS: u32 = 0x14;
    HEARTBEAT_MAX_GAP_MS: u32 = 0x1b;
    LINK_TIMEOUT_MS: u32 = 0x50;
    LINK_TIMEOUT_MAX_MS: u32 = 0x64;
    ACK_TIMEOUT_MS: u32 = 0x32;
    ACK_WINDOW: u8 = 0x40;
    HELLO_RESEND_MS: u32 = 0x64;
    APP_TIMEOUT_MS_DEFAULT: u16 = 0x03e8;
    APP_PING_DIVISOR: u8 = 0x04;
    STATUS_PERIOD_MS: u32 = 0x32;
    STATUS_MIN_GAP_MS: u32 = 0x0a;
    CONSOLE_POLL_STALL_MS: u32 = 0x64;
    QUEUE_CAP_DEFAULT: u16 = 0x0200;
    T_US_LIMIT: u32 = 0x80000000;
    STOP_NEUTRAL_MS: u32 = 0x64;
}
