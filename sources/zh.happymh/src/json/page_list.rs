use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, String, Vec},
	error,
	imports::net::Request,
	prelude::format,
	Page, Result,
};

pub struct PageList;

fn push_url_if_present(value: &serde_json::Value, out: &mut Vec<String>) {
	let url = value
		.as_str()
		.map(|s| s.to_string())
		.or_else(|| {
			value.as_object().and_then(|o| {
				o.get("url")
					.or_else(|| o.get("src"))
					.or_else(|| o.get("image"))
					.or_else(|| o.get("img"))
					.or_else(|| o.get("originalUrl"))
					.or_else(|| o.get("original_url"))
					.and_then(|v| v.as_str())
					.map(|s| s.to_string())
			})
		})
		.unwrap_or_default();

	if !url.is_empty() {
		out.push(url);
	}
}

fn collect_urls(value: &serde_json::Value, out: &mut Vec<String>, depth: u8) {
	if depth == 0 {
		return;
	}

	match value {
		serde_json::Value::Array(arr) => {
			for v in arr {
				// Many APIs return an array of either string URLs or objects containing a URL field.
				push_url_if_present(v, out);
				collect_urls(v, out, depth - 1);
			}
		}
		serde_json::Value::Object(map) => {
			for v in map.values() {
				collect_urls(v, out, depth - 1);
			}
		}
		_ => {}
	}
}

impl PageList {
	pub fn get_pages(_manga_id: String, chapter_id: String) -> Result<Vec<Page>> {
		let url = format!(
			"{}/v2.0/apis/manga/reading?code={}&v=v3.1818134",
			BASE_URL, chapter_id
		);
		let json: serde_json::Value = Request::get(url.clone())?
			.header(
				"Referer",
				&format!("{}/mangaread/{}/{}", BASE_URL, _manga_id.clone(), chapter_id.clone()),
			)
			.header("Origin", BASE_URL)
			.header("X-Requested-With", "XMLHttpRequest")
			.send()?
			.get_json()?;
		let data = json
			.as_object()
			.ok_or_else(|| error!("Expected JSON object"))?;
		let data = data
			.get("data")
			.and_then(|v| v.as_object())
			.ok_or_else(|| error!("Expected data object"))?;

		let mut urls: Vec<String> = Vec::new();

		// Try common field names first.
		if let Some(v) = data.get("scans").or_else(|| data.get("items")).or_else(|| data.get("images")).or_else(|| data.get("list")) {
			collect_urls(v, &mut urls, 6);
		}

		// Fallback: the API changes frequently; recursively walk the `data` object and
		// pick up any URL-like fields or arrays of URLs.
		if urls.is_empty() {
			collect_urls(&serde_json::Value::Object(data.clone()), &mut urls, 6);
		}

		if urls.is_empty() {
			return Err(error!("Expected page list array").into());
		}

		let mut pages: Vec<Page> = Vec::new();
		for url in urls {
			pages.push(Page {
				content: aidoku::PageContent::url(url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}
