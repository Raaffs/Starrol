use std::error::Error;

use k256::{ecdsa::{Signature, SigningKey, VerifyingKey, signature::{hazmat::{PrehashSigner, PrehashVerifier}}}};
use crate::batcher::signer::DigitalSignatureService;
pub struct Secp256k1 {
    private_key: SigningKey,
}

impl Secp256k1 {
    pub fn new(bytes: &[u8;32])->Result<Self, Box<dyn Error>>{
        let signing_key=SigningKey::from_slice(bytes).
        map_err(|e| format!("failed to generate secp256k1 key from bytes {e}"))?;
        Ok(Self{private_key: signing_key})
    }
}

impl DigitalSignatureService for Secp256k1{
    fn sign(&self, msg_hash: &[u8; 32]) -> Result<Vec<u8>, Box<dyn Error>> {
        let sig: Signature=self.
        private_key.
        sign_prehash(msg_hash).
        map_err(|e| e.to_string())?;
        Ok(sig.to_vec())
    }

    fn verify(&self, public_key_bytes: &[u8], msg_hash: &[u8; 32], sig: &[u8]) -> Result<bool, Box<dyn Error>> {
        let vk = VerifyingKey::from_sec1_bytes(public_key_bytes)?;
        let signature = Signature::from_slice(sig)?;
        Ok(vk.verify_prehash(msg_hash, &signature).is_ok())
    }
}