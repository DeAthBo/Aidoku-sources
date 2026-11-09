#![cfg_attr(not(test), no_std)]

mod crypto;
mod html;
mod models;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, string::ToString as _},
	imports::net::Request,
	prelude::*,
};

struct Dumanwu;

impl Source for Dumanwu {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		_page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			let keyword: String = query.trim().chars().take(12).collect();
			let body = net::form_pair("k", &keyword);
			let url = format!("{}/s", net::DESKTOP_URL);
			let document = net::post_form(&url, body, net::DESKTOP_URL, false)?.html()?;
			return Ok(html::parse_search(&document));
		}

		for filter in &filters {
			if let FilterValue::Select { id, value } = filter {
				if value.is_empty() {
					continue;
				}
				let url = match id.as_str() {
					"ranking" => Some(format!("{}/rank/{value}", net::DESKTOP_URL)),
					"category" => Some(format!("{}/sort/{value}", net::DESKTOP_URL)),
					_ => None,
				};
				if let Some(url) = url {
					let document = net::get(&url)?.html()?;
					return Ok(html::parse_ranking(&document));
				}
			}
		}

		let document = net::get(&format!("{}/rank/1", net::DESKTOP_URL))?.html()?;
		Ok(html::parse_ranking(&document))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let key = manga.key.trim_matches('/').to_string();
		let url = format!("{}/{key}/", net::DESKTOP_URL);
		let document = net::get(&url)?.html()?;
		if needs_details {
			html::update_details(&document, &mut manga);
		}
		if needs_chapters {
			manga.chapters = Some(html::parse_chapters(&document, &key)?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		html::pages(&chapter.key)
	}
}

impl ListingProvider for Dumanwu {
	fn get_manga_list(&self, listing: Listing, _page: i32) -> Result<MangaPageResult> {
		let id = match listing.id.as_str() {
			"popular" => "1",
			"latest" => "5",
			_ => bail!("Invalid listing: `{}`", listing.id),
		};
		let document = net::get(&format!("{}/rank/{id}", net::DESKTOP_URL))?.html()?;
		Ok(html::parse_ranking(&document))
	}
}

impl ImageRequestProvider for Dumanwu {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("User-Agent", net::USER_AGENT)
			.header("Referer", net::BASE_URL))
	}
}

impl DeepLinkHandler for Dumanwu {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split_once("dumanwu1.com")
			.map_or(url.as_str(), |(_, path)| path)
			.trim_start_matches('/')
			.trim_end_matches('/');
		if path.is_empty() {
			return Ok(None);
		}
		let mut parts = path.split('/');
		let manga_key = parts.next().unwrap_or_default();
		if ["rank", "sort", "s", "morechapter", "static"].contains(&manga_key) {
			return Ok(None);
		}
		let result = if let Some(chapter) = parts.next() {
			let chapter = chapter.strip_suffix(".html").unwrap_or(chapter);
			DeepLinkResult::Chapter {
				manga_key: manga_key.to_string(),
				key: format!("{manga_key}/{chapter}"),
			}
		} else {
			DeepLinkResult::Manga {
				key: manga_key.to_string(),
			}
		};
		Ok(Some(result))
	}
}

register_source!(
	Dumanwu,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
