pub struct ScriptOpcodes;

impl ScriptOpcodes {
    pub const OP_DUP: u8 = 0x76;
    pub const OP_HASH160: u8 = 0xA9;
    pub const OP_EQUALVERIFY: u8 = 0x88;
    pub const OP_CHECKSIG: u8 = 0xAC;
}
