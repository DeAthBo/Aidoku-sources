use aidoku::{
	Result,
	alloc::{String, string::ToString as _},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};

pub const BASE_URL: &str = "http://m.dumanwu1.com";
pub const DESKTOP_URL: &str = "http://www.dumanwu1.com";

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn get(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8"))
}

pub fn post_form(url: &str, body: String, referer: &str, xhr: bool) -> Result<Request> {
	let mut request = Request::post(url)?
		.header("User-Agent", USER_AGENT)
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
		.header("Referer", referer);
	if xhr {
		request = request.header("X-Requested-With", "XMLHttpRequest");
	}
	Ok(request.body(body))
}

pub fn form_pair(name: &str, value: &str) -> String {
	let mut params = QueryParameters::new();
	params.push(name, Some(value));
	params.to_string()
}

pub fn absolute(href: &str) -> String {
	// The source domains have invalid TLS certificates (Chrome: privacy error),
	// so their own URLs must be plain http. Third-party CDN images, however,
	// return 301 on http and are valid over https, so their scheme is kept.
	if let Some(rest) = href.strip_prefix("https://") {
		if rest.starts_with("m.dumanwu1.com") || rest.starts_with("www.dumanwu1.com") {
			format!("http://{rest}")
		} else {
			href.to_string()
		}
	} else if href.starts_with("http://") {
		href.to_string()
	} else if let Some(rest) = href.strip_prefix("//") {
		format!("http://{rest}")
	} else if href.starts_with('/') {
		format!("{BASE_URL}{href}")
	} else {
		href.to_string()
	}
}
