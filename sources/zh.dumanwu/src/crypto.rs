use aidoku::alloc::{String, Vec};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

const KEYS: [&[u8]; 10] = [
	b"smkhy258",
	b"smkd95fv",
	b"md496952",
	b"cdcsdwq",
	b"vbfsa256",
	b"cawf151c",
	b"cd56cvda",
	b"8kihnt9",
	b"dso15tlo",
	b"5ko6plhy",
];

/// Unpack a Dean Edwards p.a.c.k.e.r script, mirroring Keiyoushi's Unpacker.
pub fn unpack(script: &str) -> Option<String> {
	let packed = script
		.split_once("}('")?
		.1
		.split_once(".split('|'),0,{}))")?
		.0
		.replace("\\'", "\"");

	let (data, rest) = packed.split_once("',")?;
	if data.is_empty() {
		return Some(String::new());
	}
	let dict_str = rest.split_once('\'')?.1.split_once('\'')?.0;
	let dictionary: Vec<&str> = dict_str.split('|').collect();

	let mut result = String::with_capacity(data.len());
	let bytes = data.as_bytes();
	let mut i = 0;
	while i < bytes.len() {
		if is_word_byte(bytes[i]) {
			let start = i;
			while i < bytes.len() && is_word_byte(bytes[i]) {
				i += 1;
			}
			let word = &data[start..i];
			let index = radix62(word);
			match dictionary.get(index) {
				Some(value) if !value.is_empty() => result.push_str(value),
				_ => result.push_str(word),
			}
		} else {
			let ch = data[i..].chars().next()?;
			result.push(ch);
			i += ch.len_utf8();
		}
	}
	Some(result)
}

fn is_word_byte(byte: u8) -> bool {
	byte.is_ascii_alphanumeric() || byte == b'_'
}

fn radix62(word: &str) -> usize {
	let mut result: usize = 0;
	for byte in word.bytes() {
		let value = if byte <= b'9' {
			(byte - b'0') as usize
		} else if byte >= b'a' {
			(byte - (b'a' - 10)) as usize
		} else {
			(byte - (b'A' - 36)) as usize
		};
		result = result.wrapping_mul(62).wrapping_add(value);
	}
	result
}

/// base64 decode -> XOR with the fixed key for `index` -> base64 decode.
pub fn decrypt(data: &str, index: usize) -> Option<String> {
	let key = *KEYS.get(index)?;
	let mut bytes = STANDARD.decode(data.trim()).ok()?;
	for (i, byte) in bytes.iter_mut().enumerate() {
		*byte ^= key[i % key.len()];
	}
	let encoded = core::str::from_utf8(&bytes).ok()?;
	let decoded = STANDARD.decode(encoded).ok()?;
	String::from_utf8(decoded).ok()
}
