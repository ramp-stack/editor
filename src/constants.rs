pub const RIGHT_PAD:              f32 = 0.0;
pub const AUTOSAVE_INTERVAL_SECS: f64 = 5.0;
pub const BLINK_IDLE_SECS:        f32 = 0.5;
pub const BLINK_RATE_SECS:        f32 = 0.53;

pub const KW: &[&str] = &[
    "fn","let","mut","pub","use","mod","struct","enum","impl","trait",
    "type","const","static","extern","crate","super","where","as","in",
    "ref","dyn","unsafe","async","await","move","true","false",
];
pub const CTRL: &[&str] = &[
    "if","else","match","for","while","loop","return","break","continue",
];