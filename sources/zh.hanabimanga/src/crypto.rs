use aidoku::{
	Result,
	alloc::{String, Vec},
	error,
	prelude::*,
};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

// The app derives the signing key from a shipped secret and its own certificate
// fingerprint, so the native client is the only one that can request image urls.
const NATIVE_SECRET: &str = "7c6cb7f919a688c4c1f5eacf047d97dcd542cae8e3fc84d926160ad879bf3845";
const CERT_FINGERPRINT: &str = "13a8c9fbdaf19115f39b48dc0f3a7c99568ebb0c91ba1a71218c76f53f7877a8";

pub fn signature(payload: &str) -> Result<String> {
	let mut mac = HmacSha256::new_from_slice(&signing_key()?)
		.map_err(|_| error!("Invalid signature key length"))?;
	mac.update(payload.as_bytes());
	Ok(hex::encode(mac.finalize().into_bytes()))
}

pub fn payload_sha256(payload: &str) -> String {
	let mut hasher = Sha256::new();
	hasher.update(payload.as_bytes());
	hex::encode(hasher.finalize())
}

fn signing_key() -> Result<Vec<u8>> {
	let secret = hex::decode(NATIVE_SECRET)
		.map_err(|_| error!("Invalid native secret"))?;
	let fingerprint = hex::decode(CERT_FINGERPRINT)
		.map_err(|_| error!("Invalid certificate fingerprint"))?;

	let mut hasher = Sha256::new();
	hasher.update(&secret);
	hasher.update(&fingerprint);
	Ok(hasher.finalize().to_vec())
}
