#![no_std]

mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContext,
	Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
	register_source,
};
use html::{ChapterPage as _, GenresPage as _, MangaList as _, MangaPage as _};
use net::{USER_AGENT, Url};

// Paths that aren't manga slugs, so that they aren't mistaken for one by deep links.
const RESERVED_PATHS: &[&str] = &["category", "search", "custom", "tags", "m", "top", "rank"];

struct Miaoqu;

impl Source for Miaoqu {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query {
			return Url::search(&query, page)
				.request()?
				.html()?
				.manga_page_result(page);
		}

		Url::category(page, &filters)
			.request()?
			.html()?
			.manga_page_result(page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// The mobile page holds both the details and the chapter list.
		let document = Url::manga(&manga.key).request()?.html()?;

		if needs_details {
			document.update_details(&mut manga);
		}

		if needs_chapters {
			manga.chapters = Some(document.chapters());
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Url::chapter(&chapter.key)
			.request()?
			.html()?
			.pages(&chapter.key)
	}
}

impl ListingProvider for Miaoqu {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "order/hits",
			"latest" => "order/addtime",
			_ => bail!("Invalid listing: `{}`", listing.id),
		};

		Url::listing(sort, page)
			.request()?
			.html()?
			.manga_page_result(page)
	}
}

impl DynamicFilters for Miaoqu {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		Ok([Url::genres().request()?.html()?.genre_filter()?.into()].into())
	}
}

impl ImageRequestProvider for Miaoqu {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
	}
}

impl DeepLinkHandler for Miaoqu {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split_once("miaoqumh.org")
			.map_or(url.as_str(), |(_, path)| path)
			.trim_start_matches('/');

		let deep_link_result = if path.ends_with(".html") {
			// Chapter urls only carry the chapter id, so the manga link has to be
			// read off the chapter page.
			let manga_key = Url::chapter(path).request()?.html()?.manga_key()?;
			Some(DeepLinkResult::Chapter {
				manga_key,
				key: path.into(),
			})
		} else if !path.is_empty() && !path.contains('/') && !RESERVED_PATHS.contains(&path) {
			Some(DeepLinkResult::Manga { key: path.into() })
		} else {
			None
		};

		Ok(deep_link_result)
	}
}

register_source!(
	Miaoqu,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);
