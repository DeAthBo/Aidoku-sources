use aidoku::{
	alloc::{String, Vec, format, string::ToString as _, vec},
	imports::std::current_date,
};
use md5::{Digest, Md5};
use num_bigint::BigUint;

/// Salt prefixed and suffixed to the gsn signature string.
pub const GSN_SALT: &str = "4e0a48e1c0b54041bce9c8f0e036124d";

/// Every request (API and images) must carry a desktop Chrome UA.
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

// RSA 2048-bit public modulus, extracted from the shipped X.509 public key.
const MODULUS_HEX: &str = "9850A0DBCF5D4F0B36EEFF06B6A21F7E43F8CE0151F8C608B9421E54EE401A2055D2B7E9461E6083B8BC46B4F5D84F63E97C0AA1EDF1273D648439CF73AE4FE5FEC225C349F40F1B8FB025E4AE23606C73FB843E9F46334A59573E2B7F573FE25B98C57653FA093220515C2F2281842F6B12E1CAAAA92A420CA1D27A25E1F42B4CB24B5CE98CAB48DFBC9906C1BE84927B0D7CAFFBC2D3B89089A6240A572CEE320FABDB6E8A52FFFF6B91F645A7E63B29F331CA563997B8BB470E133D94E6D9182549C1D94DD8A0CA4E1DBBEAEFE9CD0D6CE6EF33FC046D2B6E8DF1EE70F2636519C1DC1DFC328637C442EAEFAE792A607DFBFB5D94DE31A4E583471B238F73";

// The upstream generates these once per device from a random source. WASM has no
// RNG, so a fixed (but valid) device identity is used for the whole source life.
const IMEI_BASE: &str = "35209900176148";
const ANDROID_ID_BYTES: [u8; 8] = [0x3A, 0x7F, 0x2B, 0x9C, 0x4E, 0x1D, 0x60, 0x85];
pub const DEVICE_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";
const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[inline]
fn keep(b: u8) -> bool {
	b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~')
}

/// Equivalent to `URLEncoder.encode(str, "UTF-8")` followed by the upstream's
/// `.replace("+", "%20").replace("%7E", "~").replace("*", "%2A")`.
/// Net effect: keep `[A-Za-z0-9._~-]`, percent-encode every other UTF-8 byte
/// with uppercase hex (space -> %20, `*` -> %2A).
pub fn url_encode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for &b in s.as_bytes() {
		if keep(b) {
			out.push(b as char);
		} else {
			out.push('%');
			out.push(HEX_UPPER[(b >> 4) as usize] as char);
			out.push(HEX_UPPER[(b & 0x0f) as usize] as char);
		}
	}
	out
}

/// Lowercase hex MD5, used for the `gsn` query parameter.
pub fn md5_hex(s: &str) -> String {
	let mut hasher = Md5::new();
	hasher.update(s.as_bytes());
	let digest = hasher.finalize();
	let mut out = String::with_capacity(32);
	for b in digest {
		out.push(HEX_LOWER[(b >> 4) as usize] as char);
		out.push(HEX_LOWER[(b & 0x0f) as usize] as char);
	}
	out
}

/// Standard Base64 (NO_WRAP), used for the RSA ciphertext.
pub fn base64_encode(data: &[u8]) -> String {
	let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
	let mut i = 0;
	while i + 3 <= data.len() {
		let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
		out.push(BASE64[((n >> 18) & 63) as usize] as char);
		out.push(BASE64[((n >> 12) & 63) as usize] as char);
		out.push(BASE64[((n >> 6) & 63) as usize] as char);
		out.push(BASE64[(n & 63) as usize] as char);
		i += 3;
	}
	match data.len() - i {
		1 => {
			let n = (data[i] as u32) << 16;
			out.push(BASE64[((n >> 18) & 63) as usize] as char);
			out.push(BASE64[((n >> 12) & 63) as usize] as char);
			out.push('=');
			out.push('=');
		}
		2 => {
			let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
			out.push(BASE64[((n >> 18) & 63) as usize] as char);
			out.push(BASE64[((n >> 12) & 63) as usize] as char);
			out.push(BASE64[((n >> 6) & 63) as usize] as char);
			out.push('=');
		}
		_ => {}
	}
	out
}

/// `RSA/ECB/PKCS1Padding` encryption with the shipped 2048-bit public key.
/// PKCS#1 v1.5 type-2 block: 0x00 0x02 PS 0x00 M, then `m^65537 mod n`.
/// PS uses fixed non-zero bytes because WASM has no random source; the padding
/// is still valid and decrypts to the same plaintext.
pub fn rsa_encrypt(message: &str) -> Option<String> {
	let n = BigUint::parse_bytes(MODULUS_HEX.as_bytes(), 16)?;
	let e = BigUint::from(65537u32);
	let m = message.as_bytes();
	if m.len() > 253 {
		return None;
	}
	let ps_len = 256 - 3 - m.len();
	let mut em: Vec<u8> = Vec::with_capacity(256);
	em.push(0x00);
	em.push(0x02);
	for i in 0..ps_len {
		let mut b = (i as u8).wrapping_mul(37).wrapping_add(11);
		if b == 0 {
			b = 0x4b;
		}
		em.push(b);
	}
	em.push(0x00);
	em.extend_from_slice(m);

	let c = BigUint::from_bytes_be(&em).modpow(&e, &n);
	let mut bytes = c.to_bytes_be();
	if bytes.len() > 256 {
		return None;
	}
	let mut padded: Vec<u8> = vec![0u8; 256 - bytes.len()];
	padded.append(&mut bytes);
	Some(base64_encode(&padded))
}

/// Upstream `addLuhnCheckDigit`: sum digits (even index as-is, odd doubled and
/// reduced), then append the check digit.
pub fn add_luhn_check_digit(s: &str) -> String {
	let mut sum = 0u32;
	for (i, c) in s.bytes().enumerate() {
		let v = (c - b'0') as u32;
		sum += if i % 2 == 0 {
			v
		} else {
			let d = v * 2;
			if d < 10 { d } else { d - 9 }
		};
	}
	let mut cd = sum % 10;
	if cd != 0 {
		cd = 10 - cd;
	}
	format!("{s}{cd}")
}

/// Fixed 15-digit IMEI (14 random digits + Luhn check digit upstream).
pub fn imei() -> String {
	add_luhn_check_digit(IMEI_BASE)
}

/// 8 random bytes -> uppercase hex with leading zeros stripped.
pub fn android_id() -> String {
	let mut hex = String::with_capacity(16);
	for b in ANDROID_ID_BYTES {
		hex.push(HEX_UPPER[(b >> 4) as usize] as char);
		hex.push(HEX_UPPER[(b & 0x0f) as usize] as char);
	}
	let trimmed = hex.trim_start_matches('0');
	if trimmed.is_empty() {
		"0".into()
	} else {
		trimmed.into()
	}
}

/// `yyyy-MM-dd+HH:mm:ss` in GMT+8, matching the upstream local-time formatter.
pub fn gts() -> String {
	let t = current_date() + 8 * 3600;
	let days = t.div_euclid(86400);
	let secs = t.rem_euclid(86400);
	let (y, mo, d) = civil_from_days(days);
	format!(
		"{:04}-{:02}-{:02}+{:02}:{:02}:{:02}",
		y,
		mo,
		d,
		secs / 3600,
		(secs % 3600) / 60,
		secs % 60
	)
}

/// Millisecond "first/last used time", rounded to the second, generated once.
pub fn last_used_time() -> String {
	(current_date() * 1000).to_string()
}

// Howard Hinnant's civil_from_days.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
	let z = z + 719468;
	let era = if z >= 0 { z } else { z - 146096 } / 146097;
	let doe = z - era * 146097;
	let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
	let y = yoe + era * 400;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let d = doy - (153 * mp + 2) / 5 + 1;
	let m = mp + if mp < 10 { 3 } else { -9 };
	(if m <= 2 { y + 1 } else { y }, m, d)
}
