#![cfg_attr(not(test), no_std)]

mod crypto;
mod models;
mod net;

use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, format, string::ToString as _, vec},
	error,
	imports::{
		defaults::{DefaultValue, defaults_get, defaults_set},
		net::Request,
	},
	prelude::*,
	register_source,
};
use core::cell::RefCell;
use models::{
	AnonyUserKey, AnonyUserRequest, ErrorResponse, MangaDetailResponse, MangaListResponse,
	PageListResponse, TokenResponse,
};
use net::PAGE_SIZE;

const USER_ID_KEY: &str = "userId";
const TOKEN_KEY: &str = "token";

struct Manhuaren {
	/// Cached anonymous token / user id (mirrors the upstream preferences).
	token: RefCell<Option<String>>,
	user_id: RefCell<Option<String>>,
	/// `generateLastUsedTime()` is lazy and fixed for the source's lifetime.
	last_used_time: RefCell<Option<String>>,
}

impl Source for Manhuaren {
	fn new() -> Self {
		Self {
			token: RefCell::new(None),
			user_id: RefCell::new(None),
			last_used_time: RefCell::new(None),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let start = (PAGE_SIZE * (page - 1)).to_string();
		let limit = PAGE_SIZE.to_string();

		let (path, extra): (&str, Vec<(String, String)>) = match query {
			Some(q) if !q.is_empty() => (
				"/v1/search/getSearchManga",
				vec![
					("start".into(), start),
					("limit".into(), limit),
					("keywords".into(), q),
				],
			),
			_ => {
				// Defaults match the upstream FilterList state 0.
				let mut sort = String::from("0");
				let mut sub_category_id = String::from("0");
				let mut sub_category_type = String::from("0");
				for filter in &filters {
					if let FilterValue::Select { id, value } = filter {
						match id.as_str() {
							"状态" if !value.is_empty() => sort = value.clone(),
							"分类" if !value.is_empty() => {
								// ids are encoded as "type:id" for the two category params.
								if let Some((t, i)) = value.split_once(':') {
									sub_category_type = t.into();
									sub_category_id = i.into();
								}
							}
							_ => {}
						}
					}
				}
				(
					"/v2/manga/getCategoryMangas",
					vec![
						("start".into(), start),
						("limit".into(), limit),
						("sort".into(), sort),
						("subCategoryId".into(), sub_category_id),
						("subCategoryType".into(), sub_category_type),
					],
				)
			}
		};

		let response: MangaListResponse = self.get(path, &extra)?;
		let entries = response.response.into_entries();
		Ok(MangaPageResult {
			has_next_page: !entries.is_empty(),
			entries,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		// A single getDetail request carries both details and chapters.
		let extra = vec![("mangaId".into(), manga.key.clone())];
		let response: MangaDetailResponse = self.get("/v1/manga/getDetail", &extra)?;
		if needs_details {
			response.response.fill_details(&mut manga);
		}
		if needs_chapters {
			manga.chapters = Some(response.response.chapters());
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let extra = vec![
			("mangaSectionId".into(), chapter.key),
			("netType".into(), "4".into()),
			("loadreal".into(), "1".into()),
			("imageQuality".into(), "2".into()),
		];
		let response: PageListResponse = self.get("/v1/manga/getRead", &extra)?;
		Ok(response.response.into_pages())
	}
}

impl ListingProvider for Manhuaren {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => "0",
			"latest" => "1",
			_ => bail!("Invalid listing: {}", listing.id),
		};
		let extra = vec![
			("subCategoryType".into(), "0".into()),
			("subCategoryId".into(), "0".into()),
			("start".into(), (PAGE_SIZE * (page - 1)).to_string()),
			("limit".into(), PAGE_SIZE.to_string()),
			("sort".into(), sort.into()),
		];
		let response: MangaListResponse = self.get("/v2/manga/getCategoryMangas", &extra)?;
		let entries = response.response.into_entries();
		Ok(MangaPageResult {
			has_next_page: !entries.is_empty(),
			entries,
		})
	}
}

impl ImageRequestProvider for Manhuaren {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		let user_id = self
			.user_id
			.borrow()
			.as_ref()
			.cloned()
			.unwrap_or_else(|| "-1".into());
		let imei = crypto::imei();
		let last = self.last();
		let headers = net::common_headers(&user_id, &imei, &last);

		let mut request = Request::get(url.as_str())?;
		for (key, value) in &headers {
			request = request.header(key.as_str(), value.as_str());
		}
		// Upstream sets the dm5 api referer on every page image request.
		Ok(request.header("Referer", "http://www.dm5.com/dm5api/"))
	}
}

impl DeepLinkHandler for Manhuaren {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// webHosts from the upstream: www.manhuaren.com, www.dm5.com, m.dm5.com.
		let known_host = url.contains("www.manhuaren.com")
			|| url.contains("www.dm5.com")
			|| url.contains("m.dm5.com");
		if !known_host {
			return Ok(None);
		}

		let manga_id = if let Some(id) = query_param(&url, "id") {
			id
		} else {
			// /manhua-.../ pages have no id query, the id lives in the page's JS.
			let html = Request::get(url.as_str())?
				.header("User-Agent", crypto::USER_AGENT)
				.string()?;
			match extract_manga_id(&html) {
				Some(id) => id,
				None => return Ok(None),
			}
		};

		Ok(Some(DeepLinkResult::Manga { key: manga_id }))
	}
}

impl Manhuaren {
	fn last(&self) -> String {
		if let Some(v) = self.last_used_time.borrow().as_ref().cloned() {
			return v;
		}
		let v = crypto::last_used_time();
		*self.last_used_time.borrow_mut() = Some(v.clone());
		v
	}

	/// `fetchToken`: use the cached/settings credentials, or register an
	/// anonymous user and cache the result.
	fn credentials(&self) -> Result<(String, String)> {
		if let (Some(t), Some(u)) = (
			self.token.borrow().as_ref().cloned(),
			self.user_id.borrow().as_ref().cloned(),
		) {
			return Ok((t, u));
		}

		let t = defaults_get::<String>(TOKEN_KEY).unwrap_or_default();
		let u = defaults_get::<String>(USER_ID_KEY).unwrap_or_default();
		if !t.is_empty() && !u.is_empty() {
			*self.token.borrow_mut() = Some(t.clone());
			*self.user_id.borrow_mut() = Some(u.clone());
			return Ok((t, u));
		}

		let (t, u) = self.create_anon_user()?;
		defaults_set(TOKEN_KEY, DefaultValue::String(t.clone()));
		defaults_set(USER_ID_KEY, DefaultValue::String(u.clone()));
		*self.token.borrow_mut() = Some(t.clone());
		*self.user_id.borrow_mut() = Some(u.clone());
		Ok((t, u))
	}

	/// POST /v1/user/createAnonyUser2 with three RSA-encrypted device keys.
	fn create_anon_user(&self) -> Result<(String, String)> {
		let keys = vec![
			AnonyUserKey {
				key: crypto::rsa_encrypt(&crypto::imei())
					.ok_or_else(|| error!("RSA encryption failed"))?,
				key_type: "0".into(),
			},
			AnonyUserKey {
				key: crypto::rsa_encrypt(&crypto::android_id())
					.ok_or_else(|| error!("RSA encryption failed"))?,
				key_type: "2".into(),
			},
			AnonyUserKey {
				key: crypto::rsa_encrypt(crypto::DEVICE_UUID)
					.ok_or_else(|| error!("RSA encryption failed"))?,
				key_type: "-1".into(),
			},
		];
		let body = serde_json::to_string(&AnonyUserRequest { keys })
			.map_err(|_| error!("Invalid anonymous request body"))?;

		let data = self.signed_request(
			"/v1/user/createAnonyUser2",
			&[],
			"POST",
			Some(&body),
			"-1",
			None,
		)?;
		let response: TokenResponse =
			serde_json::from_slice(&data).map_err(|_| error!("Invalid token response"))?;
		Ok((
			format!(
				"{} {}",
				response.response.token_result.scheme, response.response.token_result.parameter
			),
			response.response.user_id.to_string(),
		))
	}

	/// Build a signed GET request, fetch it, and run the error interceptor.
	fn get<T: serde::de::DeserializeOwned>(
		&self,
		path: &str,
		extra: &[(String, String)],
	) -> Result<T> {
		let (token, user_id) = self.credentials()?;
		let data = self.signed_request(path, extra, "GET", None, &user_id, Some(&token))?;
		serde_json::from_slice(&data).map_err(|_| error!("Invalid response JSON"))
	}

	fn signed_request(
		&self,
		path: &str,
		extra: &[(String, String)],
		method: &str,
		body: Option<&str>,
		user_id: &str,
		token: Option<&str>,
	) -> Result<Vec<u8>> {
		let imei = crypto::imei();
		let last = self.last();
		let mut params = net::base_params(user_id, &imei, &crypto::gts(), &last);
		for (name, value) in extra {
			params.set(name, value);
		}

		let url = net::build_url(path, &params, method, body);
		let mut headers = net::common_headers(user_id, &imei, &last);
		if let Some(t) = token {
			headers.push(("Authorization".into(), t.into()));
		}
		if body.is_some() {
			headers.push(("Content-Type".into(), "application/json".into()));
		}

		let mut request = if method == "POST" {
			Request::post(url.as_str())?
		} else {
			Request::get(url.as_str())?
		};
		for (key, value) in &headers {
			request = request.header(key.as_str(), value.as_str());
		}
		if let Some(b) = body {
			request = request.body(b.as_bytes());
		}

		let data = request.send()?.get_data()?;
		self.check_error(&data)?;
		Ok(data)
	}

	/// `ErrorResponseInterceptor`: clear credentials on any `errorResponse`.
	fn check_error(&self, data: &[u8]) -> Result<()> {
		if let Ok(response) = serde_json::from_slice::<ErrorResponse>(data)
			&& response.error_response.is_some()
		{
			defaults_set(TOKEN_KEY, DefaultValue::Null);
			defaults_set(USER_ID_KEY, DefaultValue::Null);
			*self.token.borrow_mut() = None;
			*self.user_id.borrow_mut() = None;
			return Err(error!(
				"请求出错，用户ID已自动清除。请尝试以下其一后再试：\n选项1：打开漫画人官方APP（不用登录账号）\n选项2：从其他设备拷贝用户ID和Token到本插件设置里"
			));
		}
		Ok(())
	}
}

/// Extract a URL query parameter value (stops at `&`, `#`, or `/`).
fn query_param(url: &str, name: &str) -> Option<String> {
	for separator in ["?", "&"] {
		let pattern = format!("{separator}{name}=");
		if let Some(pos) = url.find(&pattern) {
			let rest = &url[pos + pattern.len()..];
			let end = rest.find(['&', '#', '/']).unwrap_or(rest.len());
			let value = &rest[..end];
			if !value.is_empty() {
				return Some(value.into());
			}
		}
	}
	None
}

/// Regex equivalent of `DM5_COMIC_MID\s*=\s*(\d+)`.
fn extract_manga_id(html: &str) -> Option<String> {
	let pos = html.find("DM5_COMIC_MID")?;
	let rest = &html[pos + "DM5_COMIC_MID".len()..];
	let mut chars = rest.chars().peekable();
	while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
		chars.next();
	}
	if chars.next() != Some('=') {
		return None;
	}
	while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
		chars.next();
	}
	let mut digits = String::new();
	while let Some(c) = chars.peek() {
		if c.is_ascii_digit() {
			digits.push(*c);
			chars.next();
		} else {
			break;
		}
	}
	if digits.is_empty() {
		None
	} else {
		Some(digits)
	}
}

register_source!(
	Manhuaren,
	ListingProvider,
	ImageRequestProvider,
	DeepLinkHandler
);
