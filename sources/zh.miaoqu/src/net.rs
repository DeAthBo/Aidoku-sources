use aidoku::{
	FilterValue, Result,
	alloc::{String, string::ToString as _},
	helpers::uri::encode_uri,
	imports::net::Request,
};
use core::fmt::{Display, Formatter, Result as FmtResult};

pub const BASE_URL: &str = "https://www.miaoqumh.org";
pub const MOBILE_URL: &str = "https://m.miaoqumh.org";

pub const USER_AGENT: &str =
	"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/121.0";

pub enum Url<'a> {
	// The category path is built from filter ids such as `finish/1` or `tags/215`,
	// all of which are already valid path segments.
	Category { path: String, page: i32 },
	Search { query: String, page: i32 },
	Manga { key: &'a str },
	Chapter { key: &'a str },
	Genres,
}

impl Url<'_> {
	pub fn request(&self) -> Result<Request> {
		Request::get(self.to_string())?
			.header("User-Agent", USER_AGENT)
	}
}

impl<'a> Url<'a> {
	pub fn category(page: i32, filters: &[FilterValue]) -> Self {
		let mut status = "";
		let mut sort = "order/hits";
		let mut tag = "";

		for filter in filters {
			if let FilterValue::Select { id, value } = filter {
				if value.is_empty() {
					continue;
				}

				match id.as_str() {
					"进度" => status = value,
					"排序" => sort = value,
					"题材" => tag = value,
					_ => continue,
				}
			}
		}

		let mut path = String::new();
		for part in [status, sort, tag] {
			if !part.is_empty() {
				path.push_str(part);
				path.push('/');
			}
		}

		Self::Category { path, page }
	}

	pub fn listing(sort: &str, page: i32) -> Self {
		let mut path = String::from(sort);
		path.push('/');
		Self::Category { path, page }
	}

	pub fn search(query: &str, page: i32) -> Self {
		Self::Search {
			query: encode_uri(query),
			page,
		}
	}

	pub const fn manga(key: &'a str) -> Self {
		Self::Manga { key }
	}

	pub const fn chapter(key: &'a str) -> Self {
		Self::Chapter { key }
	}

	pub const fn genres() -> Self {
		Self::Genres
	}
}

impl Display for Url<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Self::Category { path, page } => write!(f, "{BASE_URL}/category/{path}page/{page}"),
			Self::Search { query, page } => write!(f, "{BASE_URL}/search/{query}/{page}"),
			Self::Manga { key } => write!(f, "{MOBILE_URL}/{key}"),
			Self::Chapter { key } => write!(f, "{BASE_URL}/{key}"),
			Self::Genres => write!(f, "{MOBILE_URL}/category/"),
		}
	}
}
