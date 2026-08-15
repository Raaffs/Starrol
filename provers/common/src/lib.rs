use bytemuck::{Pod, Zeroable};
//  (160 bytes total = 40 RISC-V words)
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct InsertItem {
    pub credential_root: [u8; 32],
    pub signature: [u8; 64],  // r (32B) + s (32B)
    pub public_key: [u8; 64], // X (32B) + Y (32B)
}

impl Default for InsertItem {
    fn default() -> Self {
        Self {
            credential_root: [0u8; 32],
            signature: [0u8; 64],
            public_key: [0u8; 64],
        }
    }
}


//  (192 bytes total = 48 RISC-V words)
// signature on current_credential_root+updated_credential_root
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct UpdateItem {
    pub current_credential_root: [u8; 32],
    pub updated_credential_root: [u8; 32],
    pub signature: [u8; 64],  // r (32B) + s (32B)
    pub public_key: [u8; 64], // X (32B) + Y (32B)
}

impl Default for UpdateItem {
    fn default() -> Self {
        Self {
            current_credential_root: [0u8; 32],
            updated_credential_root: [0u8; 32],
            signature: [0u8; 64],
            public_key: [0u8; 64],
        }
    }
}

// []StateTransitionProof required to be sent with UpdateItem
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct StateTransitionProof{
    pub updated_indices:u32,
    pub current_state_proof: [u8;32],
    pub next_state_proof: [u8;32],
}

impl Default for StateTransitionProof {
    fn default() -> Self {
        Self {
            updated_indices:0,
            current_state_proof: [0u8; 32],
            next_state_proof: [0u8; 32],
        }
    }
}
