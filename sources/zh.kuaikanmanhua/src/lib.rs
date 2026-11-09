#![cfg_attr(not(test), no_std)]

mod models;
mod net;
mod nuxt;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, format},
	imports::net::Request,
	prelude::*,
	register_source,
};
use serde_json::Value;

struct Kuaikanmanhua;

impl Source for Kuaikanmanhua {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			let url = net::search_url(&query, page);
			let value: Value = net::api(&url)?.json_owned()?;
			Ok(models::api_search_result(&value))
		} else {
			let url = net::tag_url(page, &filters);
			let value = net::nuxt_json(&url)?;
			Ok(models::web_list_result(&value))
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = net::topic_url(&manga.key);
		let value = net::nuxt_json(&url)?;
		if needs_details {
			models::update_details(&value, &mut manga);
		}
		if needs_chapters {
			manga.chapters = Some(models::chapters(&value));
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let path = chapter.key.replace("/web/comic/", "/webs/comic-next/");
		let url = format!("{}{path}", net::BASE_URL);
		if let Ok(value) = net::nuxt_json(&url) {
			let pages = models::pages(&value);
			if !pages.is_empty() {
				return Ok(pages);
			}
		}

		// The official comic endpoint carries the same images without the Nuxt
		// payload; used when the reader page is unavailable or undecodable.
		let cid = chapter.key.rsplit('/').next().unwrap_or_default();
		let value: Value = net::api(&net::api_comic_url(cid))?.json_owned()?;
		let pages = models::api_comic_pages(&value);
		if pages.is_empty() {
			bail!("No pages found");
		}
		Ok(pages)
	}
}

impl ListingProvider for Kuaikanmanhua {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "2",
			"latest" => "3",
			_ => bail!("Invalid listing: `{}`", listing.id),
		};
		let value = net::nuxt_json(&net::listing_url(sort, page))?;
		Ok(models::web_list_result(&value))
	}
}

impl ImageRequestProvider for Kuaikanmanhua {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		net::image(url)
	}
}

impl DeepLinkHandler for Kuaikanmanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		if !url.contains("kuaikanmanhua.com") {
			return Ok(None);
		}
		let id = url
			.split_once("/web/topic/")
			.map(|(_, rest)| rest)
			.or_else(|| url.split_once("/mobile/").map(|(_, rest)| rest))
			.and_then(|rest| rest.split(['/', '?', '#']).next())
			.filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
		let Some(id) = id else {
			return Ok(None);
		};
		Ok(Some(DeepLinkResult::Manga {
			key: String::from(id),
		}))
	}
}

register_source!(
	Kuaikanmanhua,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
