use aes::Aes128;
use aidoku::{Result, alloc::Vec, imports::canvas::ImageRef};
use cbc::Decryptor;
use cbc::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};

/// `Base64.decode("NlgrYjYuRT5ic1hifSs9Tg==")` — a 16 byte AES-128 key.
const KEY: &[u8] = b"6X+b6.E>bsXb}+=N";
const IV_SIZE: usize = 16;

/// Mirrors `aesDecryptWithPrependedIv(key, ivSize = 16)`: the response starts
/// with a 16 byte IV followed by AES-128-CBC ciphertext with PKCS#7 padding.
fn decrypt_aes_cbc(data: &[u8]) -> Option<Vec<u8>> {
	if data.len() <= IV_SIZE || !(data.len() - IV_SIZE).is_multiple_of(16) {
		return None;
	}
	let mut buf = data[IV_SIZE..].to_vec();
	let decryptor = Decryptor::<Aes128>::new_from_slices(KEY, &data[..IV_SIZE]).ok()?;
	decryptor
		.decrypt_padded_mut::<Pkcs7>(&mut buf)
		.ok()
		.map(|plain| plain.to_vec())
}

fn is_image(data: &[u8]) -> bool {
	data.starts_with(b"\x89PNG\r\n\x1a\n")
		|| data.starts_with(b"\xff\xd8\xff")
		|| data.starts_with(b"GIF87a")
		|| data.starts_with(b"GIF89a")
		|| data.starts_with(b"BM")
		|| (data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP")
}

/// Aidoku has no OkHttp interceptor, so the encrypted-image check is done here
/// instead of by inspecting a `#true` URL fragment. Pages carry the upstream
/// `encrypted-image` flag in their context; covers have no context and are
/// auto-detected by trying the decrypt and verifying the result's magic bytes.
pub fn decrypt_image(image: ImageRef, encrypted: Option<bool>) -> Result<ImageRef> {
	if encrypted == Some(false) {
		return Ok(image);
	}
	let data = image.data();
	if data.len() <= IV_SIZE {
		return Ok(image);
	}
	let Some(plain) = decrypt_aes_cbc(&data) else {
		return Ok(image);
	};
	if encrypted == Some(true) || is_image(&plain) {
		Ok(ImageRef::new(&plain))
	} else {
		Ok(image)
	}
}
