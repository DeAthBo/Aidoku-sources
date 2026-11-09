use aidoku::{
	Result,
	alloc::{String, format},
	helpers::uri::encode_uri_component,
	imports::net::Request,
};

pub const BASE_URL: &str = "https://bx.yaimer.com";
pub const API_URL: &str = "https://bx.yaimer.com/api";

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Every API call is an XHR from the site, so it carries the desktop Chrome UA
/// plus the headers the web app sends. Images reuse the UA and Referer.
fn with_headers(request: Request) -> Request {
	request
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
		.header("Accept", "application/json, text/plain, */*")
		.header("X-Requested-With", "XMLHttpRequest")
}

pub fn get(url: &str) -> Result<Request> {
	Ok(with_headers(Request::get(url)?))
}

pub fn post_json(path: &str, body: &[u8]) -> Result<Request> {
	Ok(with_headers(Request::post(format!("{API_URL}{path}"))?)
		.header("Content-Type", "application/json")
		.body(body))
}

pub fn image_url(path: &str) -> String {
	format!("{API_URL}{path}")
}

pub fn manga_url(id: &str) -> String {
	format!("{API_URL}/manga/common/get?id={}", encode_uri_component(id))
}

pub fn chapter_pages_url(chapter_id: &str) -> String {
	format!(
		"{API_URL}/manga/common/getChapterList?chapterId={}",
		encode_uri_component(chapter_id)
	)
}

pub fn category_tree_url() -> String {
	format!("{API_URL}/manga/common/categoryTree")
}

pub fn manga_detail_page_url(id: &str) -> String {
	format!("{BASE_URL}/manga/detail/{id}")
}

pub fn chapter_page_url(id: &str) -> String {
	format!("{BASE_URL}/manga/chapter/{id}")
}
