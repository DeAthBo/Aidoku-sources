#![cfg_attr(not(test), no_std)]

mod html;
mod models;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};

struct CManhua;

impl Source for CManhua {
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
			let results: Vec<models::SearchDto> =
				net::get(&net::search_url(&query))?.json_owned()?;
			let entries = results
				.into_iter()
				.map(|item| {
					let url = net::manga_url(&item.slug);
					Manga {
						key: item.slug,
						title: item.title,
						cover: Some(net::absolute(&item.cover)),
						url: Some(url),
						..Default::default()
					}
				})
				.collect();
			return Ok(MangaPageResult {
				entries,
				has_next_page: false,
			});
		}

		let mut sort = "updated_desc";
		let mut status = "";
		for filter in &filters {
			if let FilterValue::Select { id, value } = filter {
				match id.as_str() {
					"sort" if !value.is_empty() => sort = value.as_str(),
					"status" if !value.is_empty() => status = value.as_str(),
					_ => {}
				}
			}
		}
		fetch_manga_list(&net::browse_url(sort, status), page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = net::manga_url(manga.key.trim_matches('/'));
		let document = net::get(&url)?.html()?;
		if needs_details {
			html::update_details(&document, &mut manga);
		}
		if needs_chapters {
			manga.chapters = Some(html::chapters(&document, &url)?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let document = net::get(&net::chapter_url(&chapter.key))?.html()?;
		Ok(html::pages(&document))
	}
}

impl ListingProvider for CManhua {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "views_desc",
			"latest" => "updated_desc",
			_ => bail!("Invalid listing: `{}`", listing.id),
		};
		fetch_manga_list(&net::browse_url(sort, ""), page)
	}
}

impl ImageRequestProvider for CManhua {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("User-Agent", net::USER_AGENT)
			.header("Referer", net::BASE_URL))
	}
}

impl DeepLinkHandler for CManhua {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split_once("cmanhua.com")
			.map_or(url.as_str(), |(_, path)| path);
		if let Some(key) = html::comic_slug(path) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
		if path.contains("ReadComic")
			&& let Some(key) = html::query_param(path, "id")
		{
			let manga_key = net::get(&net::chapter_url(&key))?
				.html()?
				.select("a[href*=/comic/]")
				.and_then(|list| list.into_iter().find_map(|element| element.attr("href")))
				.and_then(|href| html::comic_slug(&href));
			if let Some(manga_key) = manga_key {
				return Ok(Some(DeepLinkResult::Chapter { manga_key, key }));
			}
		}
		Ok(None)
	}
}

fn fetch_manga_list(url: &str, page: i32) -> Result<MangaPageResult> {
	let mut document = net::get(url)?.html()?;
	let mut current = 1;
	while current < page {
		let target = match html::next_page_target(&document) {
			Some(target) => target,
			None => {
				return Ok(MangaPageResult {
					entries: Vec::new(),
					has_next_page: false,
				});
			}
		};
		document = html::post_back(&document, &target, url)?;
		current += 1;
	}
	Ok(html::parse_manga_list(&document))
}

register_source!(
	CManhua,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
