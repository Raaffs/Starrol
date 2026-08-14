use bytemuck::{Pod, Zeroable};


//  (192 bytes total = 48 RISC-V words)

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct UpdateItem {
    pub old_root: [u8; 32],
    pub new_root: [u8; 32],
    pub signature: [u8; 64],  // r (32B) + s (32B)
    pub public_key: [u8; 64], // X (32B) + Y (32B)
}

impl Default for UpdateItem {
    fn default() -> Self {
        Self {
            old_root: [0u8; 32],
            new_root: [0u8; 32],
            signature: [0u8; 64],
            public_key: [0u8; 64],
        }
    }
}

//  (160 bytes total = 40 RISC-V words)
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct InsertItem {
    pub root: [u8; 32],
    pub signature: [u8; 64],  // r (32B) + s (32B)
    pub public_key: [u8; 64], // X (32B) + Y (32B)
}

impl Default for InsertItem {
    fn default() -> Self {
        Self {
            root: [0u8; 32],
            signature: [0u8; 64],
            public_key: [0u8; 64],
        }
    }
}

