use std::error::Error;

pub trait DigitalSignatureService {
    fn sign(&self, msg_hash: &[u8; 32]) -> Result<Vec<u8>, Box<dyn Error>>;
    fn verify(&self, public_key_bytes: &[u8], msg_hash: &[u8; 32], sig: &[u8]) -> Result<bool, Box<dyn Error>>;
}























