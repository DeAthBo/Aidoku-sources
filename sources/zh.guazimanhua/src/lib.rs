#![no_std]

mod html;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
	register_source,
};
use html::{ChapterPage as _, MangaList as _, MangaPage as _};
use net::{BASE_URL, Url};

struct Guazimanhua;

impl Source for Guazimanhua {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		Url::category(query.as_deref(), page, &filters)
			.request()?
			.html()?
			.manga_page_result()
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// Both the details and the chapter list are on the same page, so this only
		// has to be fetched once even when everything is needed.
		let document = Url::key(&manga.key).request()?.html()?;

		if needs_details {
			document.update_details(&mut manga);
		}

		if needs_chapters {
			manga.chapters = Some(document.chapters());
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		Ok(Url::key(&chapter.key).request()?.html()?.pages())
	}
}

impl ListingProvider for Guazimanhua {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "hits",
			"latest" => "update",
			_ => bail!("Invalid listing: `{}`", listing.id),
		};

		Url::listing(sort, page)
			.request()?
			.html()?
			.manga_page_result()
	}
}

impl ImageRequestProvider for Guazimanhua {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", BASE_URL))
	}
}

impl DeepLinkHandler for Guazimanhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split_once("guazimanhua.com")
			.map_or(url.as_str(), |(_, path)| path);
		let key = path.trim_start_matches('/');

		let deep_link_result = if key.starts_with("comic.php") {
			Some(DeepLinkResult::Manga { key: key.into() })
		} else if key.starts_with("chapter.php") {
			// Chapter urls only carry the chapter id, so the manga link has to be
			// read off the reader page.
			let manga_key = Url::key(key).request()?.html()?.manga_key()?;
			Some(DeepLinkResult::Chapter {
				manga_key,
				key: key.into(),
			})
		} else {
			None
		};

		Ok(deep_link_result)
	}
}

register_source!(
	Guazimanhua,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
