use aidoku::alloc::{String, Vec, format, vec};

use crate::crypto;

pub const BASE_URL: &str = "https://mangaapi.manhuaren.com";
pub const WEBSITE_URL: &str = "https://www.manhuaren.com";
pub const PAGE_SIZE: i32 = 20;

const PACKAGE_NAME: &str = "com.ilike.cartoon";

/// A list of query parameters kept as decoded (name, value) pairs. The URL is
/// built from these with percent-encoding, while the gsn signature uses the
/// same decoded values with the upstream's `urlEncode`.
#[derive(Default)]
pub struct Params {
	pub inner: Vec<(String, String)>,
}

impl Params {
	pub fn new() -> Self {
		Self::default()
	}

	/// Set (replace) a query parameter.
	pub fn set(&mut self, name: &str, value: &str) {
		self.inner.retain(|(n, _)| n != name);
		self.inner.push((name.into(), value.into()));
	}
}

/// All the fixed `g*` device/app parameters from `myRequest`.
pub fn base_params(user_id: &str, imei: &str, gts: &str, last: &str) -> Params {
	let mut p = Params::new();
	p.set("gsm", "md5");
	p.set("gft", "json");
	p.set("gak", "android_manhuaren2");
	p.set("gat", "");
	p.set("gui", user_id);
	p.set("gts", gts);
	p.set("gut", "0");
	p.set("gem", "1");
	p.set("gaui", user_id);
	p.set("gln", "");
	p.set("gcy", "US");
	p.set("gle", "zh");
	p.set("gcl", "dm5");
	p.set("gos", "1");
	p.set("gov", "33_13");
	p.set("gav", "7.0.1");
	p.set("gdi", imei);
	p.set("gfcl", "dm5");
	p.set("gfut", last);
	p.set("glut", last);
	p.set("gpt", PACKAGE_NAME);
	p.set("gciso", "us");
	p.set("glot", "");
	p.set("glat", "");
	p.set("gflot", "");
	p.set("gflat", "");
	p.set("glbsaut", "0");
	p.set("gac", "");
	p.set("gcut", "GMT+8");
	p.set("gfcc", "");
	p.set("gflg", "");
	p.set("glcn", "");
	p.set("glcc", "");
	p.set("gflcc", "");
	p
}

fn sorted_entries<'a>(params: &'a Params, body: Option<&'a str>) -> Vec<(&'a str, &'a str)> {
	let mut entries: Vec<(&str, &str)> = params
		.inner
		.iter()
		.map(|(n, v)| (n.as_str(), v.as_str()))
		.collect();
	if let Some(b) = body {
		entries.push(("body", b));
	}
	entries.sort_by(|a, b| a.0.cmp(b.0));
	entries
}

/// `gsn = md5(salt + METHOD + Σ sorted(name + urlEncode(value)) + salt)`.
/// For POST the raw request body is added as a `body` entry.
pub fn gsn(params: &Params, method: &str, body: Option<&str>) -> String {
	let entries = sorted_entries(params, body);
	let mut s = String::from(crypto::GSN_SALT);
	s.push_str(method);
	for (name, value) in &entries {
		if *name == "gsn" {
			continue;
		}
		s.push_str(name);
		s.push_str(&crypto::url_encode(value));
	}
	s.push_str(crypto::GSN_SALT);
	crypto::md5_hex(&s)
}

/// Build a signed URL; `gsn` is appended last (it is not part of the signature).
pub fn build_url(path: &str, params: &Params, method: &str, body: Option<&str>) -> String {
	let signature = gsn(params, method, body);
	let entries = sorted_entries(params, None);
	let mut url = String::from(BASE_URL);
	url.push_str(path);
	url.push('?');
	for (i, (name, value)) in entries.iter().enumerate() {
		if i > 0 {
			url.push('&');
		}
		url.push_str(name);
		url.push('=');
		url.push_str(&crypto::url_encode(value));
	}
	url.push_str("&gsn=");
	url.push_str(&signature);
	url
}

/// Headers from `configureHeaders`. `Authorization` is added by the caller for
/// GET requests (it is only available after anonymous registration).
pub fn common_headers(user_id: &str, imei: &str, last: &str) -> Vec<(String, String)> {
	let yqci = format!(
		r#"{{"at":-1,"av":"7.0.1","ciso":"us","cl":"dm5","cy":"US","di":"{imei}","dm":"Pixel","fcl":"dm5","ft":"mhr","fut":"{last}","installation":"dm5","le":"zh","ln":"","lut":"{last}","nt":3,"os":1,"ov":"33_13","pt":"com.ilike.cartoon","rn":"1080x1920","st":0}}"#
	);
	let yqpp = r#"{"ciso":"us","laut":"0","lot":"","lat":"","cut":"GMT+8","fcc":"","flg":"","lcc":"","lcn":"","flcc":"","flot":"","flat":"","ac":""}"#;
	vec![
		("User-Agent".into(), crypto::USER_AGENT.into()),
		("X-Yq-Yqci".into(), yqci),
		("X-Yq-Key".into(), user_id.into()),
		("yq_is_anonymous".into(), "1".into()),
		("x-request-id".into(), crypto::DEVICE_UUID.into()),
		("X-Yq-Yqpp".into(), yqpp.into()),
	]
}
