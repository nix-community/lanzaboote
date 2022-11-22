use anyhow::Result;
use ed25519_compact::{KeyPair, Noise, PublicKey, SecretKey, Seed, Signature};

pub fn generate_key() -> KeyPair {
    KeyPair::from_seed(Seed::default())
}

pub fn sign(message: &[u8], private_key: &str) -> Result<Signature> {
    let private_key = SecretKey::from_pem(private_key)?;
    let signature = private_key.sign(message, Some(Noise::generate()));

    Ok(signature)
}

pub fn verify(message: &[u8], signature: &[u8], public_key: &str) -> Result<()> {
    let signature = Signature::from_slice(signature)?;
    let public_key = PublicKey::from_pem(public_key)?;

    public_key.verify(message, &signature)?;

    Ok(())
}
