#![allow(unused)]

pub mod linux {
    // These values are coming from <linux/input-event-codes.h>.
    pub mod input_event_codes {
        pub const BTN_LEFT: u32 = 0x110;
        pub const BTN_RIGHT: u32 = 0x111;
        pub const BTN_MIDDLE: u32 = 0x112;
        pub const BTN_SIDE: u32 = 0x113;
        pub const BTN_EXTRA: u32 = 0x114;
        pub const BTN_FORWARD: u32 = 0x115;
        pub const BTN_BACK: u32 = 0x116;
    }
}
