use anyhow::{Result, anyhow, bail};
use argon2::Argon2;
use chacha20poly1305::{
	Key, XChaCha20Poly1305, XNonce,
	aead::{Aead, KeyInit},
};
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroizing;

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;

/// Encrypted on database setup and decrypted on unlock to verify the master password.
pub const VERIFY_TEXT: &str = "psu::master-key::v1";

pub fn generate_salt() -> [u8; SALT_LEN] {
	let mut salt = [0u8; SALT_LEN];
	rand::rngs::OsRng.fill_bytes(&mut salt);
	salt
}

pub fn derive_key(master: &SecretString, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>> {
	let mut key = [0u8; KEY_LEN];
	Argon2::default()
		.hash_password_into(master.expose_secret().as_bytes(), salt, &mut key)
		.map_err(|_| anyhow!("Key derivation failed"))?;
	Ok(Zeroizing::new(key))
}

#[derive(Clone)]
pub struct Cipher(XChaCha20Poly1305);

impl Cipher {
	pub fn new(key: &Zeroizing<[u8; KEY_LEN]>) -> Self {
		Self(XChaCha20Poly1305::new(Key::from_slice(&key[..])))
	}

	/// Returns nonce prepended to the ciphertext.
	pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>> {
		let mut nonce_bytes = [0u8; NONCE_LEN];
		rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

		let mut ciphertext = self
			.0
			.encrypt(XNonce::from_slice(&nonce_bytes), plaintext.as_bytes())
			.map_err(|_| anyhow!("Encryption failed"))?;
		ciphertext.splice(..0, nonce_bytes);

		Ok(ciphertext)
	}

	pub fn decrypt(&self, data: &[u8]) -> Result<Zeroizing<String>> {
		let Some((nonce, ciphertext)) = data.split_at_checked(NONCE_LEN) else {
			bail!("Ciphertext too short");
		};

		let plaintext = self
			.0
			.decrypt(XNonce::from_slice(nonce), ciphertext)
			.map_err(|_| anyhow!("Decryption failed (wrong master password or corrupted data)"))?;

		Ok(Zeroizing::new(String::from_utf8(plaintext).map_err(|_| anyhow!("Invalid UTF-8 in plaintext"))?))
	}
}

impl Drop for Cipher {
	fn drop(&mut self) {
		// Overwrite the key material held by the cipher state.
		self.0 = XChaCha20Poly1305::new(Key::from_slice(&[0u8; KEY_LEN]));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn cipher() -> (Cipher, Zeroizing<[u8; KEY_LEN]>) {
		let salt = generate_salt();
		let master = SecretString::new("correct horse battery staple".into());
		let key = derive_key(&master, &salt).unwrap();
		(Cipher::new(&key), key)
	}

	#[test]
	fn encrypt_decrypt_roundtrip() {
		let (cipher, _) = cipher();
		let ciphertext = cipher.encrypt("hunter2").unwrap();

		assert_ne!(ciphertext, b"hunter2");
		assert_eq!(cipher.decrypt(&ciphertext).unwrap().as_str(), "hunter2");
	}

	#[test]
	fn wrong_key_fails_decryption() {
		let (cipher, _) = cipher();
		let ciphertext = cipher.encrypt("hunter2").unwrap();

		let other_master = SecretString::new("wrong password".into());
		let other_key = derive_key(&other_master, &generate_salt()).unwrap();
		let other_cipher = Cipher::new(&other_key);

		assert!(other_cipher.decrypt(&ciphertext).is_err());
	}

	#[test]
	fn short_ciphertext_rejected() {
		let (cipher, _) = cipher();
		assert!(cipher.decrypt(&[1, 2, 3]).is_err());
	}
}
