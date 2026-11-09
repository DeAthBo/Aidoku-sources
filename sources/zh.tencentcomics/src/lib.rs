#![cfg_attr(not(test), no_std)]

mod decode;
mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, format},
	imports::net::Request,
	prelude::*,
	register_source,
};

struct TencentComics;

impl Source for TencentComics {
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
			let doc = net::mobile_search(&query, page)?.html()?;
			html::search_page_result(&doc)
		} else {
			let url = net::filter_url(page, &filters);
			let doc = net::desktop(&url)?.html()?;
			html::popular_page_result(&doc)
		}
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// Details and chapters live on the same desktop page.
		let url = net::details_url(&manga.key);
		let doc = net::desktop(&url)?.html()?;

		if needs_details {
			html::update_details(&doc, &mut manga);
		}
		if needs_chapters {
			manga.chapters = Some(html::chapters(&doc));
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = net::chapter_url(&chapter.key);
		let document = net::desktop(&url)?.string()?;
		decode::decode_pages(&document)
	}
}

impl ListingProvider for TencentComics {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = match listing.id.as_str() {
			"popular" => net::popular_url(page),
			"latest" => net::latest_url(page),
			_ => bail!("Invalid listing: `{}`", listing.id),
		};
		let doc = net::desktop(&url)?.html()?;
		html::popular_page_result(&doc)
	}
}

impl ImageRequestProvider for TencentComics {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		net::image(url)
	}
}

impl DeepLinkHandler for TencentComics {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let marker = if url.contains("/comic/index/id/") {
			"/comic/index/id/"
		} else if url.contains("/Comic/comicInfo/id/") {
			"/Comic/comicInfo/id/"
		} else {
			return Ok(None);
		};
		let Some(id) = url
			.split_once(marker)
			.and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
			.filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
		else {
			return Ok(None);
		};

		Ok(Some(DeepLinkResult::Manga {
			key: format!("id/{id}"),
		}))
	}
}

register_source!(
	TencentComics,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
