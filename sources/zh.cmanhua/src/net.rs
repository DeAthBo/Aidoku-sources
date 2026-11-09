use aidoku::{
	Result,
	alloc::{String, string::ToString as _},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};

pub const BASE_URL: &str = "https://cmanhua.com";

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn get(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8"))
}

pub fn post_form(url: &str, body: String, referer: &str) -> Result<Request> {
	Ok(Request::post(url)?
		.header("User-Agent", USER_AGENT)
		.header("Content-Type", "application/x-www-form-urlencoded")
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
		.header("Referer", referer)
		.header("X-Requested-With", "XMLHttpRequest")
		.body(body))
}

pub fn browse_url(sort: &str, status: &str) -> String {
	let mut params = QueryParameters::new();
	params.push("orderBy", Some(sort));
	if !status.is_empty() {
		params.push("status", Some(status));
	}
	format!("{BASE_URL}/Browse?{params}")
}

pub fn search_url(query: &str) -> String {
	let mut params = QueryParameters::new();
	params.push("q", Some(query.trim()));
	format!("{BASE_URL}/Modules/Search/SearchHandler.ashx?{params}")
}

pub fn manga_url(key: &str) -> String {
	format!("{BASE_URL}/comic/{key}")
}

pub fn chapter_url(key: &str) -> String {
	format!("{BASE_URL}/ReadComic?id={key}")
}

pub fn absolute(href: &str) -> String {
	if href.starts_with("http://") || href.starts_with("https://") {
		href.to_string()
	} else if let Some(rest) = href.strip_prefix("//") {
		format!("https://{rest}")
	} else if let Some(rest) = href.strip_prefix('/') {
		format!("{BASE_URL}/{rest}")
	} else {
		format!("{BASE_URL}/{href}")
	}
}
