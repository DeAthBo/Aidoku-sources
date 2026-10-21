use aidoku::{
	Result,
	alloc::{String, format},
	imports::{defaults::defaults_get, net::Request},
};

/// Traditional Chinese site root. Every page request goes through this prefix so
/// metadata, chapters and images all come from the localized site.
pub const BASE_URL: &str = "https://www.webtoons.com/zh-hant";
pub const BASE_URL_MOBILE: &str = "https://m.webtoons.com";
/// Reported on chapters and matched against `info.languages`.
pub const LANGUAGE: &str = "zh";
/// Cookies and query parameters use the lowercase form of the site's path prefix.
const LOCALE: &str = "zh-hant";
/// Language prefixes that may appear in front of a site link. Only unambiguous
/// ones are listed so ordinary path segments are never mistaken for a locale.
const LOCALES: [&str; 3] = ["en", "zh-hant", "zh-hans"];

/// Returns the User-Agent header string.
pub fn get_user_agent(mobile: bool) -> &'static str {
	if mobile {
		"Mozilla/5.0 (iPhone; CPU iPhone OS 16_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.1 Mobile/15E148 Safari/604.1"
	} else {
		"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
	}
}

/// Request wrapper that sets cookies, referer, and user-agent.
pub fn request(url: &str, mobile: bool) -> Result<Request> {
	let cookie_string = format!(
		"locale={LOCALE}; ageGatePass=true; needGDPR=false; needCCPA=true; needCOPPA=false"
	);

	Ok(Request::get(url)?
		.header("Referer", BASE_URL)
		.header("Cookie", &cookie_string)
		.header("User-Agent", get_user_agent(mobile)))
}

/// Resolves a link from the site against the localized root. Links usually come
/// with a language prefix of their own, which is replaced by ours so pages are
/// never fetched from another locale.
pub fn absolute_url(path: &str) -> String {
	if path.starts_with("http") {
		return String::from(path);
	}
	if let Some(rest) = strip_locale(path) {
		return format!("{BASE_URL}/{rest}");
	}
	format!("{BASE_URL}{path}")
}

/// Returns the part of `path` following a leading `/<locale>/` segment.
fn strip_locale(path: &str) -> Option<&str> {
	let rest = path.strip_prefix('/')?;
	let (segment, tail) = rest.split_once('/')?;
	LOCALES.contains(&segment).then_some(tail)
}

/// Extracts a query parameter value from a URL, stopping at '&', '#', or '/'.
fn get_param<'a>(url: &'a str, param: &str) -> Option<&'a str> {
	let idx = url.find(param)?;
	let after = &url[idx + param.len()..];
	let end = after.find(['&', '#', '/']).unwrap_or(after.len());
	let val = &after[..end];
	if val.is_empty() { None } else { Some(val) }
}

fn format_id(url: &str, val: &str) -> String {
	if (url.contains("canvas") || url.contains("challenge")) && !val.ends_with("-canvas") {
		format!("{val}-canvas")
	} else {
		String::from(val)
	}
}

/// Returns the ID of a manga from a URL.
pub fn get_manga_id(url: &str) -> Option<String> {
	let val = get_param(url, "title_no=").or_else(|| get_param(url, "titleNo="))?;
	Some(format_id(url, val))
}

/// Returns the ID of a chapter from a URL.
pub fn get_chapter_id(url: &str) -> Option<String> {
	let val = get_param(url, "episode_no=").or_else(|| get_param(url, "episodeNo="))?;
	Some(format_id(url, val))
}

/// Returns full URL of a manga from a manga ID.
pub fn get_manga_url(manga_id: &str) -> String {
	if let Some(canvas_id) = manga_id.strip_suffix("-canvas") {
		format!("{BASE_URL}/challenge/episodeList?titleNo={canvas_id}")
	} else {
		format!("{BASE_URL}/episodeList?titleNo={manga_id}")
	}
}

/// Returns full URL of a chapter from a chapter ID and manga ID.
pub fn get_chapter_url(chapter_id: &str, manga_id: &str) -> String {
	let clean_manga_id = manga_id.strip_suffix("-canvas").unwrap_or(manga_id);
	let clean_chapter_id = chapter_id.strip_suffix("-canvas").unwrap_or(chapter_id);

	if manga_id.ends_with("-canvas") || chapter_id.ends_with("-canvas") {
		format!(
			"{BASE_URL}/challenge/viewer?titleNo={clean_manga_id}&episodeNo={clean_chapter_id}"
		)
	} else {
		format!("{BASE_URL}/viewer?titleNo={manga_id}&episodeNo={chapter_id}")
	}
}

/// Returns whether canvas series are enabled in settings.
pub fn get_canvas_series() -> bool {
	defaults_get::<bool>("canvasSeries").unwrap_or(true)
}

/// Returns whether optional pages should be displayed.
pub fn get_optional_pages() -> bool {
	defaults_get::<bool>("optionalPages").unwrap_or(true)
}
