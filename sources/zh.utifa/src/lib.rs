#![cfg_attr(not(test), no_std)]

mod models;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent,
	PageContext, Result, SelectFilter, SortFilter, SortFilterDefault, Source,
	alloc::{String, Vec, borrow::Cow, string::ToString},
	imports::{net::Request, std::current_date},
	prelude::*,
	register_source,
};
use core::cell::RefCell;
use models::{
	ApiResponse, CategoryDto, ChapterDto, ChapterPagesDto, MangaDto, MangaListBody, MangaPageDto,
};
use serde::de::DeserializeOwned;

const PAGE_SIZE: i32 = 24;
const RESIGN_TTL: i64 = 25 * 60;

pub struct Utifa {
	categories: RefCell<Vec<CategoryDto>>,
	resigned: RefCell<Option<Resigned>>,
}

struct Resigned {
	chapter_id: String,
	fetched_at: i64,
	urls: Vec<String>,
}

/// The API wraps every payload in `{ message, data }`; a missing `data` is an error.
fn parse_data<T: DeserializeOwned>(request: Request) -> Result<T> {
	let response: ApiResponse<T> = request.json_owned()?;
	match response.data {
		Some(data) => Ok(data),
		None => Err(error!(
			"Utifa API error: {}",
			response.message.unwrap_or_default()
		)),
	}
}

impl Source for Utifa {
	fn new() -> Self {
		Self {
			categories: RefCell::new(Vec::new()),
			resigned: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		self.ensure_categories();
		let (order_by, order, theme, grade, update_status) = {
			let categories = self.categories.borrow();
			let mut order_by = String::from("views");
			let mut order = String::from("desc");
			let mut theme = None;
			let mut grade = None;
			let mut update_status = None;
			for filter in filters {
				match filter {
					FilterValue::Sort {
						index, ascending, ..
					} => {
						if let Some(category) = categories.iter().find(|c| c.key == "orderBy")
							&& let Some(option) = category.value.get(index as usize)
						{
							order_by = option.value.clone();
						}
						order = if ascending { "asc" } else { "desc" }.into();
					}
					FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
						"theme" => theme = Some(value),
						"grade" => grade = Some(value),
						"updateStatus" => update_status = Some(value),
						_ => {}
					},
					_ => {}
				}
			}
			(order_by, order, theme, grade, update_status)
		};
		let keyword = query
			.map(|query| query.trim().to_string())
			.filter(|query| !query.is_empty());
		self.manga_list(MangaListBody {
			page_num: page,
			page_size: PAGE_SIZE,
			keyword,
			order_by,
			order,
			theme,
			grade,
			update_status,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let data: MangaDto = parse_data(net::get(&net::manga_url(&manga.key))?)?;
		if needs_details {
			data.apply_details(&mut manga);
		}
		if needs_chapters {
			let mut chapters: Vec<Chapter> =
				data.chapters.iter().map(ChapterDto::to_chapter).collect();
			// The API returns oldest-first; Aidoku expects newest-first.
			chapters.reverse();
			models::number_chapters(&mut chapters);
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chapter_id = chapter.key;
		let data: ChapterPagesDto = parse_data(net::get(&net::chapter_pages_url(&chapter_id))?)?;
		let urls: Vec<String> = data
			.chapters
			.iter()
			.map(|file| net::image_url(&file.url))
			.collect();
		let pages = urls
			.iter()
			.enumerate()
			.map(|(index, url)| {
				let mut context = PageContext::new();
				context.insert(String::from("chapter"), chapter_id.clone());
				context.insert(String::from("index"), index.to_string());
				Page {
					content: PageContent::url_context(url.clone(), context),
					..Default::default()
				}
			})
			.collect();
		*self.resigned.borrow_mut() = Some(Resigned {
			chapter_id,
			fetched_at: current_date(),
			urls,
		});
		Ok(pages)
	}
}

impl ListingProvider for Utifa {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let order_by = match listing.id.as_str() {
			"popular" => "views",
			"latest" => "updateTime",
			_ => bail!("Invalid listing: `{}`", listing.id),
		};
		self.manga_list(MangaListBody {
			page_num: page,
			page_size: PAGE_SIZE,
			keyword: None,
			order_by: order_by.into(),
			order: String::from("desc"),
			theme: None,
			grade: None,
			update_status: None,
		})
	}
}

impl DynamicFilters for Utifa {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let categories = match self.fetch_categories() {
			Ok(categories) => categories,
			Err(_) => return Ok(Vec::new()),
		};
		let filters = build_filters(&categories);
		*self.categories.borrow_mut() = categories;
		Ok(filters)
	}
}

impl ImageRequestProvider for Utifa {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let resolved = match context {
			Some(context) => {
				let chapter_id = context.get("chapter");
				let index: Option<usize> =
					context.get("index").and_then(|value| value.parse().ok());
				match (chapter_id, index) {
					(Some(chapter_id), Some(index)) => self
						.resigned_url(chapter_id, index)
						.unwrap_or_else(|| url.clone()),
					_ => url.clone(),
				}
			}
			None => url.clone(),
		};
		Ok(Request::get(&resolved)?
			.header("User-Agent", net::USER_AGENT)
			.header("Referer", net::BASE_URL))
	}
}

impl DeepLinkHandler for Utifa {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some((_, path)) = url.split_once("bx.yaimer.com") else {
			return Ok(None);
		};
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments: Vec<&str> = path
			.split('/')
			.filter(|segment| !segment.is_empty())
			.collect();
		let result = match segments.as_slice() {
			["manga", "detail", id] if id.parse::<i64>().is_ok() => Some(DeepLinkResult::Manga {
				key: (*id).to_string(),
			}),
			_ => None,
		};
		Ok(result)
	}
}

impl Utifa {
	fn manga_list(&self, body: MangaListBody) -> Result<MangaPageResult> {
		let json = serde_json::to_vec(&body).map_err(|_| error!("Invalid manga list body"))?;
		let data: MangaPageDto = parse_data(net::post_json("/manga/common/list", &json)?)?;
		Ok(MangaPageResult {
			entries: data.records.into_iter().map(MangaDto::into_manga).collect(),
			has_next_page: data.current < data.pages,
		})
	}

	fn ensure_categories(&self) {
		if !self.categories.borrow().is_empty() {
			return;
		}
		if let Ok(categories) = self.fetch_categories() {
			*self.categories.borrow_mut() = categories;
		}
	}

	fn fetch_categories(&self) -> Result<Vec<CategoryDto>> {
		parse_data(net::get(&net::category_tree_url())?)
	}

	/// Page links are signed for 30 minutes. Re-fetch fresh links after 25 minutes
	/// (RESIGN_TTL) to leave a safety margin before expiry, mirroring the upstream
	/// OkHttp interceptor.
	fn resigned_url(&self, chapter_id: &str, index: usize) -> Option<String> {
		let now = current_date();
		{
			let cache = self.resigned.borrow();
			if let Some(resigned) = cache.as_ref()
				&& resigned.chapter_id == chapter_id
				&& now - resigned.fetched_at < RESIGN_TTL
			{
				return resigned.urls.get(index).cloned();
			}
		}
		let data: ChapterPagesDto =
			parse_data(net::get(&net::chapter_pages_url(chapter_id)).ok()?).ok()?;
		let urls: Vec<String> = data
			.chapters
			.iter()
			.map(|file| net::image_url(&file.url))
			.collect();
		*self.resigned.borrow_mut() = Some(Resigned {
			chapter_id: chapter_id.into(),
			fetched_at: now,
			urls: urls.clone(),
		});
		urls.get(index).cloned()
	}
}

fn build_filters(categories: &[CategoryDto]) -> Vec<Filter> {
	categories
		.iter()
		.map(|category| {
			let options: Vec<Cow<'static, str>> = category
				.value
				.iter()
				.map(|option| Cow::Owned(option.label.clone()))
				.collect();
			if category.key == "orderBy" {
				let default_index = category
					.value
					.iter()
					.position(|option| option.value == "views")
					.unwrap_or(0) as i32;
				SortFilter {
					id: Cow::Borrowed("orderBy"),
					title: Some(Cow::Owned(category.label.clone())),
					can_ascend: true,
					options,
					default: Some(SortFilterDefault {
						index: default_index,
						ascending: false,
					}),
					..Default::default()
				}
				.into()
			} else {
				let ids: Vec<Cow<'static, str>> = category
					.value
					.iter()
					.map(|option| {
						Cow::Owned(if option.value == "all" {
							String::new()
						} else {
							option.value.clone()
						})
					})
					.collect();
				SelectFilter {
					id: Cow::Owned(category.key.clone()),
					title: Some(Cow::Owned(category.label.clone())),
					is_genre: true,
					options,
					ids: Some(ids),
					..Default::default()
				}
				.into()
			}
		})
		.collect()
}

register_source!(
	Utifa,
	ListingProvider,
	DeepLinkHandler,
	DynamicFilters,
	ImageRequestProvider
);
