use aidoku::{
	Result,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::encode_uri_component,
	imports::{net::Request, std::current_date},
};

pub const BASE_URL: &str = "https://vomicmh.com";
pub const API_URL: &str = "https://api.vomicmh.com/pics_new/pics";

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

const SALT: &str = "hhhasda";
const INITIAL_HASH: u32 = 0x9E37_79B9;

/// Standard base64 (with padding, no line wraps), matching `Base64.NO_WRAP`.
fn base64_encode(input: &[u8]) -> String {
	const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
	let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
	let mut i = 0;
	while i + 3 <= input.len() {
		let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | (input[i + 2] as u32);
		out.push(TABLE[((n >> 18) & 63) as usize] as char);
		out.push(TABLE[((n >> 12) & 63) as usize] as char);
		out.push(TABLE[((n >> 6) & 63) as usize] as char);
		out.push(TABLE[(n & 63) as usize] as char);
		i += 3;
	}
	let rem = input.len() - i;
	if rem == 1 {
		let n = (input[i] as u32) << 16;
		out.push(TABLE[((n >> 18) & 63) as usize] as char);
		out.push(TABLE[((n >> 12) & 63) as usize] as char);
		out.push('=');
		out.push('=');
	} else if rem == 2 {
		let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
		out.push(TABLE[((n >> 18) & 63) as usize] as char);
		out.push(TABLE[((n >> 12) & 63) as usize] as char);
		out.push(TABLE[((n >> 6) & 63) as usize] as char);
		out.push('=');
	}
	out
}

/// Pure-Rust port of `SignInterceptor.hash`: a 32-bit rotate/xor mixing
/// function seeded by the salt, formatted as an unsigned 8-digit hex string.
fn hash(data: &str, timestamp: &str) -> String {
	let key = SALT.as_bytes();
	let mut h = INITIAL_HASH;
	for &byte in key {
		h = (h ^ byte as u32).rotate_left(7);
	}
	let mut input = Vec::new();
	input.extend_from_slice(data.as_bytes());
	input.extend_from_slice(timestamp.as_bytes());
	input.extend_from_slice(key);
	for (index, &byte) in input.iter().enumerate() {
		let k = key[index % key.len()] as u32;
		let mix = if index % 2 == 0 { k * 8 } else { k >> 1 };
		h = (h ^ byte as u32).rotate_left(5) ^ mix;
	}
	for &byte in key {
		h = (h ^ (byte as u32 * 16)).rotate_left(9);
	}
	format!("{h:08x}")
}

/// Builds the URL and the canonical payload used by the signature.
/// The URL holds every parameter (even empty ones), while the payload only
/// keeps non-empty values sorted by name, exactly like the Kotlin interceptor.
fn build_url(path: &str, params: &[(String, String)]) -> (String, String) {
	// `queryParameterValues(name).lastOrNull()` keeps the last value per name.
	let mut pairs: Vec<(&str, &str)> = Vec::with_capacity(params.len());
	for (name, value) in params {
		let name = name.as_str();
		let value = value.as_str();
		if let Some(position) = pairs.iter().position(|(existing, _)| *existing == name) {
			pairs[position].1 = value;
		} else {
			pairs.push((name, value));
		}
	}

	let mut url = String::from(API_URL);
	url.push_str(path);
	for (index, (name, value)) in pairs.iter().enumerate() {
		url.push(if index == 0 { '?' } else { '&' });
		url.push_str(&encode_uri_component(*name));
		url.push('=');
		url.push_str(&encode_uri_component(*value));
	}

	let mut signed: Vec<(&str, &str)> = pairs
		.iter()
		.copied()
		.filter(|(_, value)| !value.is_empty())
		.collect();
	signed.sort_by(|a, b| a.0.cmp(b.0));
	let mut payload = String::new();
	for (name, value) in &signed {
		if !payload.is_empty() {
			payload.push('&');
		}
		payload.push_str(name);
		payload.push('=');
		payload.push_str(value);
	}
	(url, payload)
}

fn sign(payload: &str) -> (String, String) {
	let timestamp = current_date().to_string();
	let encoded = base64_encode(format!("{payload}{timestamp}{SALT}").as_bytes());
	let signature = hash(&encoded, &timestamp);
	(timestamp, signature)
}

/// Headers the app sends on every API call, plus the `t`/`s` signature pair.
fn with_headers(request: Request, payload: &str, token: Option<&str>) -> Request {
	let (timestamp, signature) = sign(payload);
	let request = request
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
		.header("platform", "android")
		.header("hide-content", "0")
		.header("name", "pics")
		.header("version", "1.2.1")
		.header("t", timestamp.as_str())
		.header("s", signature.as_str());
	match token {
		Some(token) => request.header("Authorization", format!("Bearer {token}").as_str()),
		None => request,
	}
}

pub fn api_get(path: &str, params: &[(String, String)], token: Option<&str>) -> Result<Request> {
	let (url, payload) = build_url(path, params);
	Ok(with_headers(Request::get(url)?, &payload, token))
}

pub fn api_post(path: &str, params: &[(String, String)], body: &[u8]) -> Result<Request> {
	let (url, payload) = build_url(path, params);
	Ok(with_headers(Request::post(url)?, &payload, None)
		.header("Content-Type", "application/json")
		.body(body))
}

/// Image URLs are already signed by the API and live on a different host, so
/// they only need the desktop UA and Referer.
pub fn image_request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL))
}

pub fn manga_page_url(id: &str) -> String {
	format!("{BASE_URL}/detail/{id}")
}

pub fn chapter_page_url(manga_id: &str, chapter_id: &str) -> String {
	format!("{BASE_URL}/chapter/{manga_id}/{chapter_id}")
}
