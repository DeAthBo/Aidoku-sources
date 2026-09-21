use aidoku::{
	FilterValue, Result,
	alloc::string::ToString as _,
	helpers::uri::QueryParameters,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

pub const BASE_URL: &str = "https://www.guazimanhua.com";

// The site serves an app download page without any images for the latest chapter when
// it detects an Android mobile user agent, so requests have to use a desktop one.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

const SORTS: &[&str] = &["daily", "hits", "update", "score"];

pub enum Url<'a> {
	Category(QueryParameters),
	Key { key: &'a str },
}

impl Url<'_> {
	pub fn request(&self) -> Result<Request> {
		Request::get(self.to_string())?.header("User-Agent", USER_AGENT)
	}
}

impl<'a> Url<'a> {
	pub fn category(query: Option<&str>, page: i32, filters: &[FilterValue]) -> Self {
		let mut params = QueryParameters::new();
		if let Some(keyword) = query {
			params.push("keyword", Some(keyword));
		}

		let mut sort = "hits";
		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				if value.is_empty() {
					continue;
				}

				match id.as_str() {
					"分类" => params.push_encoded("cid", Some(value)),
					"地区" => params.push_encoded("city", Some(value)),
					"受众" => params.push_encoded("audience", Some(value)),
					"进度" => params.push_encoded("is_end", Some(value)),
					_ => continue,
				}
			} else if let FilterValue::Sort { id, index, .. } = filter
				&& id.as_str() == "排序"
				&& let Some(value) = SORTS.get(*index as usize)
			{
				sort = value;
			}
		}

		params.push_encoded("sort", Some(sort));
		params.push_encoded("page", Some(&page.to_string()));
		Self::Category(params)
	}

	pub fn listing(sort: &str, page: i32) -> Self {
		let mut params = QueryParameters::new();
		params.push_encoded("sort", Some(sort));
		params.push_encoded("page", Some(&page.to_string()));
		Self::Category(params)
	}

	pub const fn key(key: &'a str) -> Self {
		Self::Key { key }
	}
}

impl Display for Url<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Self::Category(params) => write!(f, "{BASE_URL}/category.php?{params}"),
			Self::Key { key } => write!(f, "{BASE_URL}/{key}"),
		}
	}
}
