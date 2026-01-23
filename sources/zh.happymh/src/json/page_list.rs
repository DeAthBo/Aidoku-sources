use crate::BASE_URL;
use aidoku::{
	alloc::{string::ToString as _, String, Vec},
	error,
	imports::net::Request,
	prelude::format,
	Page, Result,
};

pub struct PageList;

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

		let list = data
			.get("scans")
			.or_else(|| data.get("items"))
			.or_else(|| data.get("images"))
			.or_else(|| data.get("list"))
			.and_then(|v| {
				v.as_array().or_else(|| {
					v.as_object()
						.and_then(|o| o.get("items"))
						.and_then(|v| v.as_array())
				})
			})
			.ok_or_else(|| error!("Expected page list array"))?;
		let mut pages: Vec<Page> = Vec::new();

		for item in list.iter() {
			let url = item
				.as_str()
				.map(|s| s.to_string())
				.or_else(|| {
					item.as_object().and_then(|o| {
						o.get("url")
							.or_else(|| o.get("src"))
							.or_else(|| o.get("image"))
							.or_else(|| o.get("img"))
							.and_then(|v| v.as_str())
							.map(|s| s.to_string())
					})
				})
				.unwrap_or_default();
			if url.is_empty() {
				continue;
			}
			pages.push(Page {
				content: aidoku::PageContent::url(url),
				..Default::default()
			});
		}

		Ok(pages)
	}
}
