use aidoku::{
	FilterValue, Result,
	alloc::{String, format},
	error,
	helpers::uri::encode_uri_component,
	imports::net::Request,
};
use serde_json::Value;

pub const BASE_URL: &str = "https://www.kuaikanmanhua.com";
pub const API_URL: &str = "https://api.kkmh.com";
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
	AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn desktop(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.header("Accept-Language", "zh-CN,zh;q=0.9"))
}

pub fn api(url: &str) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL)
		.header("X-Requested-With", "XMLHttpRequest"))
}

pub fn image(url: String) -> Result<Request> {
	Ok(Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Referer", BASE_URL))
}

/// Fetch a Nuxt SSR page and decode its compressed `__NUXT__` payload.
pub fn nuxt_json(url: &str) -> Result<Value> {
	let html = desktop(url)?.string()?;
	let script = extract_nuxt_script(&html).ok_or_else(|| error!("NUXT script not found"))?;
	let json = crate::nuxt::decode(script)?;
	serde_json::from_str(&json).map_err(|_| error!("Invalid NUXT JSON"))
}

fn extract_nuxt_script(html: &str) -> Option<&str> {
	let start = html.find("window.__NUXT__")?;
	let end = start + html[start..].find("</script>")?;
	Some(html[start..end].trim())
}

pub fn topic_url(key: &str) -> String {
	format!("{BASE_URL}/web/topic/{key}")
}

pub fn listing_url(sort: &str, page: i32) -> String {
	format!("{BASE_URL}/tag/0?region=1&pays=0&state=0&sort={sort}&page={page}")
}

pub fn tag_url(page: i32, filters: &[FilterValue]) -> String {
	let mut genre = "0";
	let mut region = "1";
	let mut pays = "0";
	let mut state = "0";
	let mut sort = "1";
	for filter in filters {
		if let FilterValue::Select { id, value } = filter
			&& !value.is_empty()
		{
			match id.as_str() {
				"题材" => genre = value.as_str(),
				"区域" => region = value.as_str(),
				"属性" => pays = value.as_str(),
				"进度" => state = value.as_str(),
				"排序" => sort = value.as_str(),
				_ => continue,
			}
		}
	}
	format!(
		"{BASE_URL}/tag/{genre}?region={region}&pays={pays}&state={state}&sort={sort}&page={page}"
	)
}

pub fn search_url(query: &str, page: i32) -> String {
	let word = encode_uri_component(query);
	let since = page.saturating_sub(1) * 10;
	format!("{API_URL}/v1/search/topic?q={word}&since={since}&size=10")
}

pub fn api_comic_url(cid: &str) -> String {
	format!("{API_URL}/v2/comic/{cid}")
}
