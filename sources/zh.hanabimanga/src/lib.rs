#![no_std]

mod crypto;
mod models;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, Result, Source,
	alloc::{String, Vec, format, string::ToString as _},
	error,
	imports::{defaults::defaults_get, std::current_date},
	prelude::*,
	register_source,
};
use core::cell::RefCell;
use models::{
	ChapterRow, ClientDiag, Comic, LoginBody, LoginResult, MetaSecurity, PageListBody, PagesResult,
	SearchBody, chapters,
};
use net::{
	ANONYMOUS_TOKEN, DEFAULT_ORDER, LATEST_ORDER, PAGE_SIZE, POPULAR_ORDER, Url, post_json,
	post_page_list,
};

struct HanabiManga {
	// Logging in only lifts the anonymous quota, so the token is optional and
	// cached for as long as the source lives.
	token: RefCell<Option<String>>,
}

impl Source for HanabiManga {
	fn new() -> Self {
		Self {
			token: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query {
			let body = SearchBody {
				search_term: &query,
				items_per_page: PAGE_SIZE.to_string(),
				page_number: page.to_string(),
			};
			let json = serde_json::to_vec(&body)
				.map_err(|_| error!("Invalid search body"))?;
			let comics: Vec<Comic> = post_json("/rest/v1/rpc/search_comics_pgroonga", &json)?
				.json_owned()?;

			return Ok(manga_page_result(comics));
		}

		let comics: Vec<Comic> = Url::comics(DEFAULT_ORDER, page, &filters)
			.request()?
			.json_owned()?;

		Ok(manga_page_result(comics))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// A single request can return both the details and the chapter list.
		let mut comic: Comic = Url::comic(&manga.key, needs_details, needs_chapters)
			.request()?
			.json_owned::<Vec<Comic>>()?
			.pop()
			.ok_or_else(|| error!("Comic not found: `{}`", manga.key))?;

		if needs_chapters {
			let comic_id = manga.key.clone();
			manga.chapters = Some(chapters(comic.chapters.take().unwrap_or_default(), &comic_id));
		}

		if needs_details {
			comic.into_details(&mut manga);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let Some((chapter_id, size)) = chapter.key.split_once('/') else {
			bail!("Invalid chapter key: `{}`", chapter.key);
		};
		let size: i32 = size
			.parse()
			.map_err(|_| error!("Invalid page count: `{size}`"))?;

		let mut pages: Vec<String> = Vec::with_capacity(size as usize);
		for index in 1..=size {
			pages.push(format!("{index:03}"));
		}

		let timestamp = current_date();
		let comic_id = manga.key;
		let payload = format!(
			"v1|comic_id={comic_id}|chapter_id={chapter_id}|pages={}|timestamp={timestamp}",
			joined(&pages)
		);
		let body = PageListBody {
			comic_id,
			chapter_id: chapter_id.into(),
			pages,
			timestamp,
			signature: crypto::signature(&payload)?,
			client_diag: ClientDiag {
				native_status: "ok".into(),
				native_fingerprint_prefix: "13a8c9fb".into(),
				canonical_payload_sha256: crypto::payload_sha256(&payload),
				app_version: "2.4.13".into(),
				app_version_code: 2041399,
			},
		};
		let json = serde_json::to_vec(&body)
			.map_err(|_| error!("Invalid page list body"))?;
		let token = self.token()?;
		let result: PagesResult = post_page_list(&json, &token)?.json_owned()?;

		Ok(result
			.urls
			.into_iter()
			.map(|page| Page {
				content: PageContent::url(page.url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for HanabiManga {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let order = match listing.id.as_str() {
			"popular" => defaults_get::<String>("popularOrder").unwrap_or_else(|| POPULAR_ORDER.into()),
			"latest" => LATEST_ORDER.into(),
			_ => bail!("Invalid listing: `{}`", listing.id),
		};

		let comics: Vec<Comic> = Url::comics(&order, page, &[]).request()?.json_owned()?;

		Ok(manga_page_result(comics))
	}
}

impl DeepLinkHandler for HanabiManga {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url
			.split_once("hanabimanga.com")
			.map_or(url.as_str(), |(_, path)| path);
		let Some(rest) = path.split_once("comic/").map(|(_, rest)| rest) else {
			return Ok(None);
		};

		let mut parts = rest.split('/');
		let Some(manga_key) = parts.next() else {
			return Ok(None);
		};

		let deep_link_result = match parts.next() {
			None => DeepLinkResult::Manga {
				key: manga_key.into(),
			},
			Some(chapter) => {
				// Chapter urls only carry the index, so the chapter id has to be
				// looked up to be able to request its pages.
				let Some(idx) = chapter.strip_prefix("chapter-") else {
					return Ok(None);
				};
				let rows: Vec<ChapterRow> =
					Url::chapter_row(manga_key, idx).request()?.json_owned()?;
				let row = rows
					.into_iter()
					.next()
					.ok_or_else(|| error!("Chapter not found: `{idx}`"))?;

				DeepLinkResult::Chapter {
					manga_key: manga_key.into(),
					key: format!("{}/{}", row.id, row.size),
				}
			}
		};

		Ok(Some(deep_link_result))
	}
}

impl HanabiManga {
	fn token(&self) -> Result<String> {
		if let Some(token) = self.token.borrow().as_ref() {
			return Ok(token.clone());
		}

		let token = self.login().unwrap_or_else(|| ANONYMOUS_TOKEN.into());
		*self.token.borrow_mut() = Some(token.clone());

		Ok(token)
	}

	fn login(&self) -> Option<String> {
		let email = defaults_get::<String>("email").filter(|email| !email.is_empty())?;
		let password = defaults_get::<String>("password").filter(|password| !password.is_empty())?;

		let body = LoginBody {
			email: &email,
			password: &password,
			gotrue_meta_security: MetaSecurity {},
		};
		let json = serde_json::to_vec(&body).ok()?;
		let result: LoginResult = post_json("/auth/v1/token?grant_type=password", &json)
			.ok()?
			.json_owned()
			.ok()?;

		Some(result.access_token)
	}
}

fn manga_page_result(comics: Vec<Comic>) -> MangaPageResult {
	let has_next_page = comics.len() == PAGE_SIZE as usize;

	MangaPageResult {
		entries: comics.into_iter().filter_map(Comic::into_manga).collect(),
		has_next_page,
	}
}

fn joined(pages: &[String]) -> String {
	let mut joined = String::new();
	for page in pages {
		if !joined.is_empty() {
			joined.push(',');
		}
		joined.push_str(page);
	}
	joined
}

register_source!(HanabiManga, ListingProvider, DeepLinkHandler);
