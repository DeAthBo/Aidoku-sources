#![cfg_attr(not(test), no_std)]

mod crypto;
mod models;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, ImageResponse,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext,
	PageImageProcessor, Result, Source,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{canvas::ImageRef, net::Request, std::parse_date},
	prelude::*,
};
use crypto::{TokenSource, decrypt_page};
use models::{
	BrowseList, BrowseResponse, ChapterItem, MangaData, ReaderResponse, find_person,
	value_to_string,
};

const BASE_URL: &str = "https://www.hikarinagi.org";
const IMAGE_BASE_URL: &str = "https://imagesp.yurari.moe";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Same order as the options in the sort filter.
const SORT_NAMES: [&str; 5] = [
	"latest_chapter_at",
	"heat",
	"created_at",
	"publication_date",
	"name",
];

struct Hikarinagi {
	tokens: TokenSource,
}

impl Source for Hikarinagi {
	fn new() -> Self {
		Self {
			tokens: TokenSource::new(),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = browse_url(page, query.as_deref(), &filters, None);
		self.parse_browse(&url)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}/api/pages/mangas/{}", manga.key);
		let data: MangaData = self.api_request(&url)?.json_owned()?;
		let MangaData {
			manga: item,
			chapters,
			people,
			tags,
		} = data;

		// The upstream stamps every chapter with the series' latest chapter time.
		let timestamp = parse_iso_seconds(item.latest_chapter_at.as_deref());

		if needs_details {
			let author =
				find_person(&people, "ORIGINAL_CREATOR").or_else(|| find_person(&people, "AUTHOR"));
			let artist = find_person(&people, "ART");
			let tag_names: Vec<String> = tags.iter().map(|tag| tag.tag.name.clone()).collect();
			let tag_names = (!tag_names.is_empty()).then_some(tag_names);
			manga.copy_from(item.into_manga(author, artist, tag_names));
		}

		if needs_chapters {
			let manga_id = manga.key.clone();
			manga.chapters = Some(parse_chapters(chapters, &manga_id, timestamp));
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let manga_id = manga.key;
		let chapter_id = chapter.key;
		let url = format!("{BASE_URL}/api/pages/mangas/reader/{manga_id}/{chapter_id}");
		let referer = format!("{BASE_URL}/mangas/{manga_id}/read/{chapter_id}");

		let response = Request::get(&url)?
			.header("User-Agent", USER_AGENT)
			.header("Accept", "application/json, text/plain, */*")
			.header("Referer", referer.as_str())
			.header("X-Requested-With", "XMLHttpRequest")
			.send()?;
		let code = response.status_code();
		if code == 401 {
			bail!("请先在 WebView 中登录");
		}
		if !(200..300).contains(&code) {
			bail!("加载章节失败（HTTP {code}）");
		}
		let reader: ReaderResponse = response.get_json_owned()?;

		Ok(reader
			.manifest
			.pages
			.into_iter()
			.filter_map(|page| {
				let pid = value_to_string(page.id)?;
				let image_url = format!(
					"{BASE_URL}/api/v3/reader/mangas/{manga_id}/chapters/{chapter_id}/pages/{pid}/content"
				);
				let mut context = PageContext::new();
				context.insert(String::from("token"), self.tokens.new_token());
				context.insert(String::from("mime"), page.mime_type);
				context.insert(
					String::from("aad"),
					format!("manga:page:{manga_id}:{chapter_id}:{pid}"),
				);
				Some(Page {
					content: PageContent::url_context(image_url, context),
					..Default::default()
				})
			})
			.collect())
	}
}

impl Hikarinagi {
	fn api_request(&self, url: &str) -> Result<Request> {
		let referer = format!("{BASE_URL}/");
		Ok(Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Accept", "application/json, text/plain, */*")
			.header("Referer", referer.as_str()))
	}

	fn parse_browse(&self, url: &str) -> Result<MangaPageResult> {
		let response: BrowseResponse = self.api_request(url)?.json_owned()?;
		let BrowseList { items, meta } = response.list;
		Ok(MangaPageResult {
			entries: items
				.into_iter()
				.map(|item| item.into_manga(None, None, None))
				.collect(),
			has_next_page: meta.page < meta.total_pages,
		})
	}
}

impl ListingProvider for Hikarinagi {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "heat:desc",
			"latest" => "latest_chapter_at:desc",
			_ => bail!("Invalid listing: {}", listing.id),
		};
		let url = browse_url(page, None, &[], Some(sort));
		self.parse_browse(&url)
	}
}

impl ImageRequestProvider for Hikarinagi {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let referer = format!("{BASE_URL}/");
		match context {
			// Reader pages need a POST whose body carries the token the server
			// encrypts the response with. Covers have no context and are plain
			// GETs against the image CDN.
			Some(context) => {
				let token = context.get("token").cloned().unwrap_or_default();
				let body = format!(r#"{{"p":"{token}"}}"#);
				Ok(Request::post(url)?
					.header("User-Agent", USER_AGENT)
					.header("Content-Type", "application/json")
					.header("Origin", BASE_URL)
					.header("Referer", referer.as_str())
					.body(body.into_bytes()))
			}
			None => Ok(Request::get(url)?
				.header("User-Agent", USER_AGENT)
				.header("Referer", referer.as_str())),
		}
	}
}

impl PageImageProcessor for Hikarinagi {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		let (Some(token), Some(aad)) = (context.get("token"), context.get("aad")) else {
			return Ok(response.image);
		};
		let data = response.image.data();
		if data.is_empty() {
			return Ok(response.image);
		}
		let decrypted = decrypt_page(token, aad, &data)?;
		Ok(ImageRef::new(&decrypted))
	}
}

impl DeepLinkHandler for Hikarinagi {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.strip_prefix(BASE_URL).unwrap_or(url.as_str());
		let path = path.split(['?', '#']).next().unwrap_or(path);
		let Some(rest) = path.strip_prefix("/mangas/") else {
			return Ok(None);
		};
		let mut parts = rest.split('/').filter(|part| !part.is_empty());
		let Some(manga_key) = parts.next() else {
			return Ok(None);
		};

		let result = match (parts.next(), parts.next()) {
			// https://www.hikarinagi.org/mangas/<id>/read/<chapter id>
			(Some("read"), Some(chapter_key)) => DeepLinkResult::Chapter {
				manga_key: manga_key.into(),
				key: chapter_key.into(),
			},
			_ => DeepLinkResult::Manga {
				key: manga_key.into(),
			},
		};
		Ok(Some(result))
	}
}

/// `/api/pages/mangas/browse` backs search, popular and latest. The upstream
/// filter order is fixed: sort, region, audience, status, decade, magazine_id.
fn browse_url(
	page: i32,
	query: Option<&str>,
	filters: &[FilterValue],
	forced_sort: Option<&str>,
) -> String {
	let page_str = page.to_string();
	let mut qs = QueryParameters::new();
	qs.push("page", Some(page_str.as_str()));
	qs.push("page_size", Some("24"));
	if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
		qs.push("search", Some(query));
	}

	let mut sort = String::from("latest_chapter_at:desc");
	for filter in filters {
		match filter {
			FilterValue::Sort {
				index, ascending, ..
			} => {
				if let Some(name) = SORT_NAMES.get(*index as usize) {
					let direction = if *ascending { "asc" } else { "desc" };
					sort = format!("{name}:{direction}");
				}
			}
			FilterValue::Select { id, value }
				if !value.is_empty()
					&& matches!(
						id.as_str(),
						"region" | "audience" | "status" | "decade" | "magazine_id"
					) =>
			{
				qs.push(id.as_str(), Some(value.as_str()));
			}
			_ => {}
		}
	}
	if let Some(forced) = forced_sort {
		sort = forced.into();
	}
	qs.push("sort", Some(sort.as_str()));
	format!("{BASE_URL}/api/pages/mangas/browse?{qs}")
}

fn parse_iso_seconds(value: Option<&str>) -> Option<i64> {
	let value = value?.trim();
	if value.is_empty() {
		return None;
	}
	for pattern in [
		"yyyy-MM-dd'T'HH:mm:ssXXX",
		"yyyy-MM-dd'T'HH:mm:ss.SSSXXX",
		"yyyy-MM-dd'T'HH:mm:ss.SSSSSSXXX",
		"yyyy-MM-dd'T'HH:mm:ss'Z'",
	] {
		if let Some(seconds) = parse_date(value, pattern) {
			return Some(seconds);
		}
	}
	None
}

fn parse_chapters(
	chapters: Vec<ChapterItem>,
	manga_id: &str,
	timestamp: Option<i64>,
) -> Vec<Chapter> {
	let mut result: Vec<Chapter> = chapters
		.into_iter()
		.enumerate()
		.map(|(index, item)| {
			let key = item.id.to_string();
			let number = item
				.number
				.as_deref()
				.and_then(|value| value.trim().parse::<f32>().ok());
			let custom_title = item.title.filter(|title| !title.trim().is_empty());
			// Fall back to the upstream "第N话" name only when the number is not
			// numeric, so Aidoku can render the real chapter number.
			let title = custom_title.or_else(|| match number {
				Some(_) => None,
				None => item.number.as_deref().map(|value| format!("第{value}话")),
			});
			Chapter {
				key: key.clone(),
				title,
				chapter_number: number.or(Some((index + 1) as f32)),
				date_uploaded: timestamp,
				scanlators: item.size.map(|size| vec![format!("{size}P")]),
				url: Some(format!("{BASE_URL}/mangas/{manga_id}/read/{key}")),
				..Default::default()
			}
		})
		.collect();
	// The API returns oldest first; Aidoku expects the newest chapter first.
	result.reverse();
	result
}

register_source!(
	Hikarinagi,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	DeepLinkHandler
);
