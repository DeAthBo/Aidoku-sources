#![cfg_attr(not(test), no_std)]

mod crypto;
mod html;

use aidoku::{
	Chapter, CoverImageProcessor, DeepLinkHandler, DeepLinkResult, FilterValue,
	ImageRequestProvider, ImageResponse, Listing, ListingProvider, Manga, MangaPageResult, Page,
	PageContent, PageContext, PageImageProcessor, Result, Source,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		canvas::ImageRef,
		defaults::defaults_get,
		html::{Document, Html},
		net::Request,
	},
	prelude::*,
};
use serde::Deserialize;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const DEFAULT_MIRROR: &str = "https://www.favcomic.com";

#[derive(Deserialize)]
struct LoginResult {
	result: Option<String>,
	msg: Option<String>,
}

struct FavComic;

impl Source for FavComic {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base = self.base_url();
		let query = query.unwrap_or_default();

		// Defaults match the upstream filter states: 全部 / 全部 / 全部 / 全部 /
		// 人气推荐 and every tag group at 全部 ("0").
		let mut manga_type = String::from("search");
		let mut origin = String::from("0");
		let mut finished = String::from("0");
		let mut free = String::from("0");
		let mut sort = String::from("1");
		let mut tag_boy = String::from("0");
		let mut tag_girl = String::from("0");
		let mut tag_picture = String::from("0");
		let mut tag_r18 = String::from("0");

		for filter in filters {
			let FilterValue::Select { id, value } = filter else {
				continue;
			};
			match id.as_str() {
				"mangaType" => manga_type = value,
				"origin" => origin = value,
				"finished" => finished = value,
				"free" => free = value,
				"sort" => sort = value,
				"tag_boy" => tag_boy = value,
				"tag_girl" => tag_girl = value,
				"tag_picture" => tag_picture = value,
				"tag_r18" => tag_r18 = value,
				_ => {}
			}
		}

		// A non-blank keyword always routes to /search; otherwise the selected
		// manga type is the path segment.
		let segment = if query.trim().is_empty() {
			manga_type.as_str()
		} else {
			"search"
		};
		let page_str = page.to_string();
		let mut qs = QueryParameters::new();
		qs.push("keyword", Some(query.as_str()));
		qs.push("origin", Some(origin.as_str()));
		qs.push("finished", Some(finished.as_str()));
		qs.push("free", Some(free.as_str()));
		qs.push("sort", Some(sort.as_str()));
		qs.push("page", Some(page_str.as_str()));
		// Upstream's TagGroup.getTag only honours the group matching the chosen
		// type, and sends nothing when 全部 is selected.
		let tag = match manga_type.as_str() {
			"boy" => tag_boy,
			"girl" => tag_girl,
			"picture" => tag_picture,
			"r18" => tag_r18,
			_ => String::new(),
		};
		if !tag.is_empty() {
			qs.push("tag", Some(tag.as_str()));
		}

		let url = format!("{base}/{segment}?{qs}");
		let doc = self.get_document(&url)?;
		Ok(html::parse_mangas_page(&doc))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base = self.base_url();
		let url = format!("{base}{}", manga.key);
		let doc = self.get_document(&url)?;
		let (mut details, chapters) = html::parse_manga_detail(&doc);

		if needs_details {
			// doc.location() is not exposed by Aidoku, so keep the key we were
			// given and record the requested page as the manga url.
			details.key = manga.key.clone();
			details.url = Some(url);
			manga.copy_from(details);
		}
		if needs_chapters {
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base = self.base_url();
		let url = format!("{base}{}", chapter.key);
		let mut doc = self.get_document(&url)?;
		let mut code = html::chapter_code(&doc);

		// code "1" means a login is required; retry once after logging in.
		if code.as_deref() == Some("1") && self.login(&base)? {
			doc = self.get_document(&url)?;
			code = html::chapter_code(&doc);
		}
		match code.as_deref() {
			Some("1") => {
				bail!("此话需在扩展设置中配置账号密码自动登录，或在 WebView 中登录才能看")
			}
			Some("3") => bail!("金币不足，请充值"),
			Some("4") => bail!("请在 WebView 中付费解锁此话"),
			Some("444") => bail!("免费额度已用完，明天零点重置"),
			_ => {}
		}

		Ok(html::page_images(&doc)
			.into_iter()
			.map(|(image_url, encrypted)| {
				let mut context = PageContext::new();
				context.insert(String::from("encrypted"), encrypted.to_string());
				Page {
					content: PageContent::url_context(image_url, context),
					..Default::default()
				}
			})
			.collect())
	}
}

impl FavComic {
	fn base_url(&self) -> String {
		defaults_get::<String>("mirror")
			.filter(|value| !value.is_empty())
			.unwrap_or_else(|| String::from(DEFAULT_MIRROR))
	}

	fn rank_type(&self) -> String {
		defaults_get::<String>("rankType")
			.filter(|value| !value.is_empty())
			.unwrap_or_else(|| String::from("1"))
	}

	fn manga_type(&self) -> String {
		defaults_get::<String>("mangaType")
			.filter(|value| !value.is_empty())
			.unwrap_or_else(|| String::from("boy-1"))
	}

	fn get_document(&self, url: &str) -> Result<Document> {
		let referer = format!("{}/", self.base_url());
		let html = Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header(
				"Accept",
				"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
			)
			.header("Referer", referer.as_str())
			.string()?;
		// Parse with an explicit base URI so `abs:` links resolve against the
		// mirror that was actually requested.
		Html::parse_with_url(html.as_bytes(), url).map_err(|_| error!("Failed to parse HTML"))
	}

	fn login(&self, base: &str) -> Result<bool> {
		let (Some(username), Some(password)) = (
			defaults_get::<String>("username").filter(|value| !value.trim().is_empty()),
			defaults_get::<String>("password").filter(|value| !value.is_empty()),
		) else {
			return Ok(false);
		};

		let mut form = QueryParameters::new();
		form.push("loginName", Some(username.as_str()));
		form.push("password", Some(password.as_str()));
		let body = form.to_string();
		let referer = format!("{base}/login");
		let result: LoginResult = Request::post(format!("{base}/login"))?
			.header("User-Agent", USER_AGENT)
			.header("Content-Type", "application/x-www-form-urlencoded")
			.header("Referer", referer.as_str())
			.header("X-Requested-With", "XMLHttpRequest")
			.body(body.into_bytes())
			.json_owned()?;

		if result.result.as_deref() == Some("success") {
			return Ok(true);
		}
		if let Some(msg) = result.msg.filter(|msg| !msg.trim().is_empty()) {
			bail!("自动登录失败: {msg}");
		}
		Ok(false)
	}
}

impl ListingProvider for FavComic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let base = self.base_url();
		match listing.id.as_str() {
			"popular" => {
				let rank = self.rank_type();
				let manga_type = self.manga_type();
				let comic_type = manga_type
					.split_once('-')
					.map_or(manga_type.as_str(), |(_, value)| value);
				let url = format!("{base}/rank?range={rank}&comicType={comic_type}&vip=0");
				let doc = self.get_document(&url)?;
				Ok(MangaPageResult {
					entries: html::parse_rank(&doc),
					has_next_page: false,
				})
			}
			"latest" => {
				let manga_type = self.manga_type();
				let segment = manga_type
					.split_once('-')
					.map_or(manga_type.as_str(), |(value, _)| value);
				let page_str = page.to_string();
				let url = format!("{base}/{segment}?page={page_str}");
				let doc = self.get_document(&url)?;
				Ok(html::parse_mangas_page(&doc))
			}
			_ => bail!("Invalid listing: {}", listing.id),
		}
	}
}

impl ImageRequestProvider for FavComic {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let referer = format!("{}/", self.base_url());
		Ok(Request::get(url)?
			.header("User-Agent", USER_AGENT)
			.header("Referer", referer.as_str())
			.header(
				"Accept",
				"image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
			))
	}
}

impl PageImageProcessor for FavComic {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let encrypted = context.and_then(|context| {
			context
				.get("encrypted")
				.map(|value| value.as_str() == "true")
		});
		crypto::decrypt_image(response.image, encrypted)
	}
}

impl CoverImageProcessor for FavComic {
	fn process_cover_image(&self, response: ImageResponse) -> Result<ImageRef> {
		crypto::decrypt_image(response.image, None)
	}
}

impl DeepLinkHandler for FavComic {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let key = html::key_from_url(&url);
		if !key.starts_with("/comic/detail/") {
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga { key }))
	}
}

register_source!(
	FavComic,
	ListingProvider,
	ImageRequestProvider,
	PageImageProcessor,
	CoverImageProcessor,
	DeepLinkHandler
);
