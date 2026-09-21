use aidoku::{
	FilterValue, Result,
	alloc::{String, format, string::ToString as _},
	helpers::uri::QueryParameters,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

pub const HOME_URL: &str = "https://web.hanabimanga.com";

// The backend is a Supabase project, so every request carries its anonymous key.
// Logging in only lifts the quota that comes with it.
const BASE_URL: &str = "https://uhkvqrxmcapgtpspglrp.moedot.net";
pub const ANONYMOUS_TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InVoa3ZxcnhtY2FwZ3Rwc3BnbHJwIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NjM5NjgzMjksImV4cCI6MjA3OTU0NDMyOX0.uuHr888lp14ObW5eWowJrHPJGgQf3sF2l7NPmFN84g4";

const COMIC_BODY: &str = "id,title,summary,cover_url,release_date,is_finished,authors,region,latest_chapter_title,tags(id,name),categories(id,name)";
const CHAPTER_BODY: &str = "chapters(id,idx,title,image_count,category,updated_at)";
const CHAPTER_ROW_BODY: &str = "id,image_count";

pub const PAGE_SIZE: i32 = 20;
pub const DEFAULT_ORDER: &str = "id.asc.nullslast";
pub const LATEST_ORDER: &str = "updated_at.desc.nullslast";
pub const POPULAR_ORDER: &str = "popularity_daily.desc.nullslast";

pub enum Url<'a> {
	Comics { query: QueryParameters },
	Comic { id: &'a str, select: String },
	ChapterRow { comic_id: &'a str, idx: &'a str },
}

impl Url<'_> {
	pub fn request(&self) -> Result<Request> {
		Ok(Request::get(self.to_string())?
			.header("apikey", ANONYMOUS_TOKEN))
	}
}

impl<'a> Url<'a> {
	pub fn comics(order: &str, page: i32, filters: &[FilterValue]) -> Self {
		let mut sort = order;
		let mut category = "";
		let mut region = "";
		let mut status = "";

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				if value.is_empty() {
					continue;
				}

				match id.as_str() {
					"分类" => category = value,
					"分区" => region = value,
					"状态" => status = value,
					"排序" => sort = value,
					_ => continue,
				}
			}
		}

		let mut query = QueryParameters::new();
		query.push_encoded("select", Some(COMIC_BODY));
		if !category.is_empty() {
			query.push_encoded("category_id", Some(&format!("eq.{category}")));
		}
		if !region.is_empty() {
			query.push_encoded("region", Some(&format!("eq.{region}")));
		}
		if !status.is_empty() {
			query.push_encoded("is_finished", Some(&format!("eq.{status}")));
		}
		query.push_encoded("order", Some(sort));
		query.push_encoded("offset", Some(&offset(page).to_string()));
		query.push_encoded("limit", Some(&PAGE_SIZE.to_string()));

		Self::Comics { query }
	}

	pub fn comic(id: &'a str, needs_details: bool, needs_chapters: bool) -> Self {
		let mut select = String::new();
		if needs_details {
			select.push_str(COMIC_BODY);
		}
		if needs_details && needs_chapters {
			select.push(',');
		}
		if needs_chapters {
			select.push_str(CHAPTER_BODY);
		}

		Self::Comic { id, select }
	}

	pub const fn chapter_row(comic_id: &'a str, idx: &'a str) -> Self {
		Self::ChapterRow { comic_id, idx }
	}
}

impl Display for Url<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Self::Comics { query } => write!(f, "{BASE_URL}/rest/v1/comics?{query}"),
			Self::Comic { id, select } => {
				write!(f, "{BASE_URL}/rest/v1/comics?id=eq.{id}&select={select}")
			}
			Self::ChapterRow { comic_id, idx } => write!(
				f,
				"{BASE_URL}/rest/v1/chapters?select={CHAPTER_ROW_BODY}&comic_id=eq.{comic_id}&idx=eq.{idx}"
			),
		}
	}
}

fn offset(page: i32) -> i32 {
	page.saturating_sub(1).max(0) * PAGE_SIZE
}

pub fn post_json(path: &str, body: &[u8]) -> Result<Request> {
	Ok(Request::post(format!("{BASE_URL}{path}"))?
		.header("apikey", ANONYMOUS_TOKEN)
		.header("Content-Type", "application/json")
		.body(body))
}

pub fn post_page_list(body: &[u8], token: &str) -> Result<Request> {
	Ok(Request::post(format!("{BASE_URL}/functions/v1/sd-image-url"))?
		.header("apikey", ANONYMOUS_TOKEN)
		.header("Authorization", &format!("Bearer {token}"))
		.header("Content-Type", "application/json")
		.body(body))
}
