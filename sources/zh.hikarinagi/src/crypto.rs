use aes_gcm::{
	Aes256Gcm, KeyInit, Nonce,
	aead::{Aead, Payload},
};
use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::std::current_date,
	prelude::*,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use core::cell::RefCell;

const TOKEN_SIZE: usize = 64;
const IV_SIZE: usize = 12;

/// The site lets the client pick the token that is used to encrypt the page
/// response, so a deterministic PRNG seeded from the clock is enough. A real
/// `SecureRandom` is not available in the WASM sandbox, and the token only has
/// to be identical in the POST body and in the decryption context.
pub struct TokenSource(RefCell<u64>);

impl TokenSource {
	pub fn new() -> Self {
		Self(RefCell::new(
			(current_date() as u64) ^ 0x9E37_79B9_7F4A_7C15,
		))
	}

	fn next_u64(&self) -> u64 {
		let mut state = self.0.borrow_mut();
		*state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
		let mut z = *state;
		z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
		z ^ (z >> 31)
	}

	/// 64 random bytes encoded as unpadded base64url, same as `newToken()`.
	pub fn new_token(&self) -> String {
		let mut bytes = [0u8; TOKEN_SIZE];
		for i in 0..TOKEN_SIZE / 8 {
			bytes[i * 8..i * 8 + 8].copy_from_slice(&self.next_u64().to_le_bytes());
		}
		URL_SAFE_NO_PAD.encode(bytes)
	}
}

impl Default for TokenSource {
	fn default() -> Self {
		Self::new()
	}
}

/// Decrypts `iv || AES-256-GCM(ciphertext)` as served by the reader API. The
/// key is the token's two halves XORed together and the chapter path is bound
/// as associated data.
pub fn decrypt_page(token_b64: &str, aad: &str, data: &[u8]) -> Result<Vec<u8>> {
	let token = URL_SAFE_NO_PAD
		.decode(token_b64)
		.map_err(|_| error!("Invalid page token"))?;
	if token.len() != TOKEN_SIZE {
		bail!("Invalid page token length");
	}

	let mut key = [0u8; TOKEN_SIZE / 2];
	for i in 0..TOKEN_SIZE / 2 {
		key[i] = token[i] ^ token[i + TOKEN_SIZE / 2];
	}

	let (iv, ciphertext) = data
		.split_at_checked(IV_SIZE)
		.ok_or_else(|| error!("Truncated page response"))?;

	Aes256Gcm::new(&key.into())
		.decrypt(
			Nonce::from_slice(iv),
			Payload {
				msg: ciphertext,
				aad: aad.as_bytes(),
			},
		)
		.map_err(|_| error!("Failed to decrypt page image"))
}
