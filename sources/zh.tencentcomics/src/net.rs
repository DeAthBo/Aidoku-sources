use aidoku::{
	FilterValue, Result,
	alloc::{String, format},
	helpers::uri::encode_uri_component,
	imports::net::Request,
};

pub const BASE_URL: &str = "https://m.ac.qq.com";
pub const DESKTOP_URL: &str = "https://ac.qq.com";

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
	AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
// The mobile search endpoint redirects to a JS-only desktop page unless a mobile
// UA is sent. This is the only request that deviates from the desktop UA rule.
pub const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) \
	AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36";

pub fn desktop(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", DESKTOP_URL)
		.header("Accept-Language", "zh-CN,zh;q=0.9"))
}

pub fn image(url: String) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", DESKTOP_URL))
}

pub fn popular_url(page: i32) -> String {
	format!("{DESKTOP_URL}/Comic/all/search/hot/page/{page}")
}

pub fn latest_url(page: i32) -> String {
	format!("{DESKTOP_URL}/Comic/all/search/time/page/{page}")
}

pub fn details_url(key: &str) -> String {
	format!("{DESKTOP_URL}/Comic/comicInfo/{key}")
}

pub fn chapter_url(key: &str) -> String {
	if key.starts_with("http") {
		String::from(key)
	} else {
		format!("{DESKTOP_URL}{key}")
	}
}

pub fn mobile_search(query: &str, page: i32) -> Result<Request> {
	let word = encode_uri_component(query);
	Ok(
		Request::get(format!("{BASE_URL}/search/result?word={word}&page={page}"))?
			.header("User-Agent", MOBILE_USER_AGENT)
			.header("Referer", BASE_URL)
			.header("Accept-Language", "zh-CN,zh;q=0.9"),
	)
}

pub fn filter_url(page: i32, filters: &[FilterValue]) -> String {
	let mut popularity = "hot/";
	let mut vip = "";
	let mut status = "";
	let mut genre = "";

	for filter in filters {
		if let FilterValue::Select { id, value } = filter {
			match id.as_str() {
				"人气排序" => popularity = value.as_str(),
				"属性" => vip = value.as_str(),
				"进度" => status = value.as_str(),
				"标签" => genre = value.as_str(),
				_ => continue,
			}
		}
	}

	let genre_part = if genre.is_empty() {
		String::new()
	} else {
		format!("theme/{genre}/")
	};

	format!("{DESKTOP_URL}/Comic/all/{genre_part}{status}search/{popularity}{vip}page/{page}")
}
