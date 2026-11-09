use aidoku::{
	Chapter, Manga, MangaStatus, Page, PageContent,
	alloc::{String, Vec, format, string::ToString as _, vec},
	imports::std::parse_date,
};
use serde::{Deserialize, Serialize};

use crate::net::WEBSITE_URL;

const NOPIC_IMAGE: &str = "http://mhfm5.tel.cdndm5.com/tag/category/nopic.jpg";
const CHAPTER_DATE_FORMAT: &str = "yyyy-MM-dd";

fn status(v: i32) -> MangaStatus {
	match v {
		1 => MangaStatus::Completed,
		0 => MangaStatus::Ongoing,
		_ => MangaStatus::Unknown,
	}
}

// ---- listing / search ----------------------------------------------------

#[derive(Deserialize)]
pub struct MangaListResponse {
	pub response: MangaListDto,
}

#[derive(Deserialize)]
pub struct MangaListDto {
	#[serde(default)]
	pub mangas: Option<Vec<MangaDto>>,
	#[serde(default)]
	pub result: Option<Vec<MangaDto>>,
}

#[derive(Deserialize)]
pub struct MangaDto {
	#[serde(rename = "mangaId")]
	pub manga_id: i32,
	#[serde(rename = "mangaName", default)]
	pub manga_name: String,
	#[serde(rename = "mangaCoverimageUrl", default)]
	pub manga_coverimage_url: String,
	#[serde(rename = "mangaIsOver", default)]
	pub manga_is_over: i32,
	#[serde(rename = "mangaAuthor", default)]
	pub manga_author: Option<String>,
}

impl MangaDto {
	pub fn into_manga(self) -> Manga {
		Manga {
			key: self.manga_id.to_string(),
			title: self.manga_name,
			cover: Some(self.manga_coverimage_url).filter(|c| !c.is_empty()),
			authors: self.manga_author.filter(|a| !a.is_empty()).map(|a| vec![a]),
			status: status(self.manga_is_over),
			url: Some(format!("{WEBSITE_URL}/showcomic/?id={}", self.manga_id)),
			..Default::default()
		}
	}
}

impl MangaListDto {
	/// Upstream uses `result ?: mangas`, i.e. search results take precedence.
	pub fn into_entries(self) -> Vec<Manga> {
		self.result
			.or(self.mangas)
			.unwrap_or_default()
			.into_iter()
			.map(MangaDto::into_manga)
			.collect()
	}
}

// ---- detail --------------------------------------------------------------

#[derive(Deserialize)]
pub struct MangaDetailResponse {
	pub response: MangaDetailDto,
}

#[derive(Deserialize)]
pub struct MangaDetailDto {
	#[serde(rename = "mangaName", default)]
	pub manga_name: String,
	#[serde(rename = "mangaTheme", default)]
	pub manga_theme: String,
	#[serde(rename = "mangaIsOver", default)]
	pub manga_is_over: i32,
	#[serde(rename = "mangaIntro", default)]
	pub manga_intro: String,
	#[serde(rename = "mangaAuthors", default)]
	pub manga_authors: Vec<String>,
	#[serde(rename = "mangaCoverimageUrl", default)]
	pub manga_coverimage_url: Option<String>,
	#[serde(rename = "mangaPicimageUrl", default)]
	pub manga_picimage_url: Option<String>,
	#[serde(rename = "shareIcon", default)]
	pub share_icon: Option<String>,
	#[serde(rename = "mangaEpisode", default)]
	pub manga_episode: Vec<ChapterDto>,
	#[serde(rename = "mangaWords", default)]
	pub manga_words: Vec<ChapterDto>,
	#[serde(rename = "mangaRolls", default)]
	pub manga_rolls: Vec<ChapterDto>,
}

#[derive(Deserialize)]
pub struct ChapterDto {
	#[serde(rename = "sectionName", default)]
	pub section_name: String,
	#[serde(rename = "sectionTitle", default)]
	pub section_title: String,
	#[serde(rename = "sectionSort", default)]
	pub section_sort: i32,
	#[serde(rename = "sectionId", default)]
	pub section_id: i32,
	#[serde(rename = "isMustPay", default)]
	pub is_must_pay: i32,
	#[serde(rename = "releaseTime", default)]
	pub release_time: String,
}

impl ChapterDto {
	fn to_chapter(&self, kind: &str, index: usize) -> Chapter {
		let mut name = String::new();
		if self.is_must_pay == 1 {
			name.push_str("(锁) ");
		}
		if kind == "mangaEpisode" {
			name.push_str("[番外] ");
		}
		name.push_str(&self.section_name);
		if !self.section_title.is_empty() {
			name.push_str(": ");
			name.push_str(&self.section_title);
		}

		// Prefer the real section sort; fall back to the position in its group.
		let number = if self.section_sort != 0 {
			self.section_sort as f32
		} else {
			(index + 1) as f32
		};

		Chapter {
			key: self.section_id.to_string(),
			title: Some(name),
			chapter_number: Some(number),
			date_uploaded: parse_date(&self.release_time, CHAPTER_DATE_FORMAT),
			url: Some(format!("{WEBSITE_URL}/m{}/", self.section_id)),
			..Default::default()
		}
	}
}

impl MangaDetailDto {
	pub fn fill_details(&self, manga: &mut Manga) {
		let cover = self.manga_coverimage_url.clone().unwrap_or_default();
		let mut thumbnail = cover.clone();
		if cover.is_empty() || cover == NOPIC_IMAGE {
			thumbnail = self
				.manga_picimage_url
				.clone()
				.filter(|s| !s.is_empty())
				.unwrap_or(cover);
		}
		if thumbnail.is_empty() {
			thumbnail = self
				.share_icon
				.clone()
				.filter(|s| !s.is_empty())
				.unwrap_or(thumbnail);
		}

		manga.title = self.manga_name.clone();
		manga.cover = Some(thumbnail).filter(|s| !s.is_empty());
		if !self.manga_authors.is_empty() {
			manga.authors = Some(self.manga_authors.clone());
		}
		let genre = self.manga_theme.replace(' ', ", ");
		if !genre.is_empty() {
			manga.tags = Some(genre.split(", ").map(String::from).collect())
				.filter(|v: &Vec<String>| !v.is_empty());
		}
		manga.status = status(self.manga_is_over);
		if !self.manga_intro.is_empty() {
			manga.description = Some(self.manga_intro.clone());
		}
	}

	/// Upstream concatenates mangaEpisode + mangaWords + mangaRolls; Aidoku
	/// expects the newest chapter first, so the merged list is sorted descending
	/// by chapter number (stable, preserving group order for ties).
	pub fn chapters(&self) -> Vec<Chapter> {
		let mut out = Vec::new();
		for (list, kind) in [
			(&self.manga_episode, "mangaEpisode"),
			(&self.manga_words, "mangaWords"),
			(&self.manga_rolls, "mangaRolls"),
		] {
			for (index, chapter) in list.iter().enumerate() {
				out.push(chapter.to_chapter(kind, index));
			}
		}
		out.sort_by(|a, b| {
			b.chapter_number
				.unwrap_or(0.0)
				.total_cmp(&a.chapter_number.unwrap_or(0.0))
		});
		out
	}
}

// ---- page list -----------------------------------------------------------

#[derive(Deserialize)]
pub struct PageListResponse {
	pub response: PageListDto,
}

#[derive(Deserialize)]
pub struct PageListDto {
	#[serde(rename = "hostList", default)]
	pub host_list: Vec<String>,
	#[serde(rename = "mangaSectionImages", default)]
	pub manga_section_images: Vec<String>,
	#[serde(default)]
	pub query: String,
}

impl PageListDto {
	/// Page url = hostList[0] + image + query (upstream `toPageList`).
	pub fn into_pages(self) -> Vec<Page> {
		let host = self.host_list.first().cloned().unwrap_or_default();
		let query = self.query;
		self.manga_section_images
			.into_iter()
			.map(|image| Page {
				content: PageContent::url(format!("{host}{image}{query}")),
				..Default::default()
			})
			.collect()
	}
}

// ---- anonymous registration ---------------------------------------------

#[derive(Deserialize)]
pub struct TokenResponse {
	pub response: TokenData,
}

#[derive(Deserialize)]
pub struct TokenData {
	#[serde(rename = "tokenResult")]
	pub token_result: TokenResult,
	#[serde(rename = "userId")]
	pub user_id: i64,
}

#[derive(Deserialize)]
pub struct TokenResult {
	pub scheme: String,
	pub parameter: String,
}

#[derive(Serialize)]
pub struct AnonyUserRequest {
	pub keys: Vec<AnonyUserKey>,
}

#[derive(Serialize)]
pub struct AnonyUserKey {
	pub key: String,
	#[serde(rename = "keyType")]
	pub key_type: String,
}

// ---- error response ------------------------------------------------------

/// The interceptor clears the cached credentials whenever this field exists.
#[derive(Deserialize)]
pub struct ErrorResponse {
	#[serde(rename = "errorResponse", default)]
	pub error_response: Option<serde_json::Value>,
}
