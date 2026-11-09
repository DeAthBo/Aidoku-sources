#![cfg_attr(not(test), no_std)]

mod models;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent,
	PageContext, Result, SelectFilter, Source,
	alloc::{String, Vec, borrow::Cow, format, string::ToString, vec},
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		net::Request,
	},
	prelude::*,
	register_source,
};
use models::{ApiResult, CategoryDto, LoginPayload, MangaDto, Pages, Token};

const MANGA_LIMIT: i32 = 20;
const SEARCH_LIMIT: i32 = 10;
const UPDATE_LIMIT: i32 = 15;

pub struct Vomic;

impl Source for Vomic {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let keyword = query.filter(|query| !query.is_empty());
		let limit = if keyword.is_some() {
			SEARCH_LIMIT
		} else {
			MANGA_LIMIT
		};
		let mut params: Vec<(String, String)> = vec![
			(String::from("page"), page.to_string()),
			(String::from("limit"), limit.to_string()),
		];
		if let Some(keyword) = keyword {
			params.push((String::from("keyword"), keyword));
		} else {
			for filter in filters {
				if let FilterValue::Select { id, value } = filter
					&& matches!(
						id.as_str(),
						"sort" | "process" | "creativity" | "region" | "dis_type" | "cat_id"
					) {
					params.push((id, value));
				}
			}
		}
		self.manga_page("/c/getCartoonList", params, limit)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let params = vec![(String::from("car_id"), manga.key.clone())];
		let api: ApiResult<MangaDto> =
			net::api_get("/c/getCartoonInfoV1", &params, None)?.json_owned()?;
		let details = api.unpack()?;
		if needs_details {
			details.apply_details(&mut manga);
		}
		if needs_chapters {
			let mut chapters: Vec<Chapter> = details
				.chapter_list
				.iter()
				.flat_map(|group| group.values.iter())
				.map(models::ChapterDto::to_chapter)
				.collect();
			models::number_chapters(&mut chapters);
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id = chapter.key.split('/').nth(1).unwrap_or(&chapter.key);
		let params = vec![(String::from("c_id"), chapter_id.to_string())];
		let token = defaults_get::<String>("token");
		let mut response = net::api_get("/c/getChapterInfo", &params, token.as_deref())?.send();
		let needs_login = match &response {
			Ok(response) => !(200..300).contains(&response.status_code()),
			Err(_) => true,
		};
		if needs_login {
			self.login()?;
			let token = defaults_get::<String>("token");
			response = net::api_get("/c/getChapterInfo", &params, token.as_deref())?.send();
		}
		let api: ApiResult<Pages> = response?.get_json_owned()?;
		let pages = api.unpack()?;
		Ok(pages
			.images
			.into_iter()
			.filter_map(|image| image.url)
			.filter(|url| !url.is_empty())
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for Vomic {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"popular" => self.manga_page(
				"/c/getCartoonList",
				vec![
					(String::from("page"), page.to_string()),
					(String::from("limit"), MANGA_LIMIT.to_string()),
					(String::from("sort"), String::from("hot_value")),
				],
				MANGA_LIMIT,
			),
			"latest" => self.manga_page(
				"/c/getUpdateBook",
				vec![
					(String::from("page"), page.to_string()),
					(String::from("limit"), UPDATE_LIMIT.to_string()),
				],
				UPDATE_LIMIT,
			),
			_ => bail!("Invalid listing: `{}`", listing.id),
		}
	}
}

impl DynamicFilters for Vomic {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		Ok(build_filters(&self.fetch_categories()))
	}
}

impl ImageRequestProvider for Vomic {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		net::image_request(&url)
	}
}

impl DeepLinkHandler for Vomic {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some((_, path)) = url.split_once("vomicmh.com") else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments: Vec<&str> = path
			.split('/')
			.filter(|segment| !segment.is_empty())
			.collect();
		let result = match segments.as_slice() {
			["detail", id] if id.parse::<i32>().is_ok() => Some(DeepLinkResult::Manga {
				key: (*id).to_string(),
			}),
			["chapter", manga_id, chapter_id] => Some(DeepLinkResult::Chapter {
				manga_key: (*manga_id).to_string(),
				key: format!("{manga_id}/{chapter_id}"),
			}),
			_ => None,
		};
		Ok(result)
	}
}

impl Vomic {
	fn manga_page(
		&self,
		path: &str,
		params: Vec<(String, String)>,
		limit: i32,
	) -> Result<MangaPageResult> {
		let api: ApiResult<Vec<MangaDto>> = net::api_get(path, &params, None)?.json_owned()?;
		let mangas = api.unpack()?;
		let has_next_page = mangas.len() >= limit as usize;
		Ok(MangaPageResult {
			entries: mangas.into_iter().map(MangaDto::into_manga).collect(),
			has_next_page,
		})
	}

	/// Chapter info is gated behind a bearer token; log in and retry on failure.
	fn login(&self) -> Result<()> {
		let email = defaults_get::<String>("email").unwrap_or_default();
		let password = defaults_get::<String>("password").unwrap_or_default();
		if email.is_empty() || password.is_empty() {
			bail!("请在设置中填写邮箱和密码以访问内容");
		}
		let payload = LoginPayload {
			identity: email,
			password,
		};
		let body = serde_json::to_vec(&payload).map_err(|_| error!("Invalid login body"))?;
		let token: Token = net::api_post("/login", &[], &body)?.json_owned()?;
		defaults_set("token", DefaultValue::String(token.token));
		Ok(())
	}

	fn fetch_categories(&self) -> Vec<CategoryDto> {
		let Ok(request) = net::api_get("/c/getCatList", &[], None) else {
			return Vec::new();
		};
		let Ok(api) = request.json_owned::<ApiResult<Vec<CategoryDto>>>() else {
			return Vec::new();
		};
		if api.code == Some(200) {
			api.data
		} else {
			Vec::new()
		}
	}
}

fn select(
	id: &'static str,
	title: &'static str,
	options: &[&'static str],
	ids: &[&'static str],
) -> Filter {
	SelectFilter {
		id: Cow::Borrowed(id),
		title: Some(Cow::Borrowed(title)),
		options: options
			.iter()
			.map(|option| Cow::Borrowed(*option))
			.collect(),
		ids: Some(ids.iter().map(|value| Cow::Borrowed(*value)).collect()),
		..Default::default()
	}
	.into()
}

fn build_filters(categories: &[CategoryDto]) -> Vec<Filter> {
	let mut filters = vec![
		select(
			"sort",
			"排序",
			&["默认", "最热", "最新"],
			&["", "hot_value", "process_at"],
		),
		select(
			"process",
			"状态",
			&["全部", "连载中", "已完结"],
			&["", "连载中", "已完结"],
		),
		select(
			"creativity",
			"来源",
			&["全部", "原创", "汉化", "转载"],
			&["", "原创", "汉化", "转载"],
		),
		select(
			"region",
			"地区",
			&["全部", "大陆", "日本", "韩国", "港台", "欧美", "其他"],
			&["", "大陆", "日本", "韩国", "港台", "欧美", "其他"],
		),
		select(
			"dis_type",
			"类型",
			&["全部", "条漫", "页漫", "其他"],
			&["", "条漫", "页漫", "其他"],
		),
	];
	if !categories.is_empty() {
		let mut options: Vec<Cow<'static, str>> = vec![Cow::Borrowed("全部")];
		let mut ids: Vec<Cow<'static, str>> = vec![Cow::Borrowed("0")];
		for category in categories {
			options.push(Cow::Owned(category.name.clone()));
			ids.push(Cow::Owned(category.id.to_string()));
		}
		filters.push(
			SelectFilter {
				id: Cow::Borrowed("cat_id"),
				title: Some(Cow::Borrowed("分类")),
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		);
	}
	filters
}

register_source!(
	Vomic,
	ListingProvider,
	DeepLinkHandler,
	DynamicFilters,
	ImageRequestProvider
);
