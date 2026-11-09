use aidoku::{
	Chapter, Manga, MangaStatus,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::std::parse_date,
};
use serde::{Deserialize, Serialize};

use crate::net::{API_URL, chapter_page_url, manga_detail_page_url};

#[derive(Deserialize)]
pub struct ApiResponse<T> {
	pub data: Option<T>,
	pub message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaListBody {
	pub page_num: i32,
	pub page_size: i32,
	pub keyword: Option<String>,
	pub order_by: String,
	pub order: String,
	pub theme: Option<String>,
	pub grade: Option<String>,
	pub update_status: Option<String>,
}

#[derive(Deserialize)]
pub struct MangaPageDto {
	pub records: Vec<MangaDto>,
	pub current: i32,
	pub pages: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaDto {
	pub id: i64,
	pub title: String,
	pub cover: Option<String>,
	pub description: Option<String>,
	pub author: Option<String>,
	pub genres: Option<Vec<String>>,
	pub update_status: Option<String>,
	#[serde(default)]
	pub chapters: Vec<ChapterDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
	pub id: i64,
	pub title: String,
	pub update_time: Option<String>,
}

#[derive(Deserialize)]
pub struct ChapterPagesDto {
	pub chapters: Vec<FileDto>,
}

#[derive(Deserialize)]
pub struct FileDto {
	pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct CategoryDto {
	pub key: String,
	pub label: String,
	pub value: Vec<CategoryOptionDto>,
}

#[derive(Deserialize, Clone)]
pub struct CategoryOptionDto {
	pub label: String,
	pub value: String,
}

impl MangaDto {
	pub fn into_manga(self) -> Manga {
		let id = self.id.to_string();
		Manga {
			key: id.clone(),
			title: self.title,
			cover: self.cover.map(|cover| format!("{API_URL}{cover}")),
			description: self.description,
			authors: self.author.map(|author| vec![author]),
			tags: self.genres,
			status: status(self.update_status.as_deref()),
			url: Some(manga_detail_page_url(&id)),
			..Default::default()
		}
	}

	pub fn apply_details(&self, manga: &mut Manga) {
		manga.key = self.id.to_string();
		manga.title = self.title.clone();
		manga.cover = self.cover.as_ref().map(|cover| format!("{API_URL}{cover}"));
		manga.description = self.description.clone();
		manga.authors = self.author.clone().map(|author| vec![author]);
		manga.tags = self.genres.clone();
		manga.status = status(self.update_status.as_deref());
		manga.url = Some(manga_detail_page_url(&self.id.to_string()));
	}
}

fn status(value: Option<&str>) -> MangaStatus {
	match value {
		Some("0") => MangaStatus::Ongoing,
		Some("1") => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	}
}

impl ChapterDto {
	pub fn to_chapter(&self) -> Chapter {
		let id = self.id.to_string();
		Chapter {
			key: id.clone(),
			title: Some(self.title.clone()),
			chapter_number: parse_chapter_number(&self.title),
			date_uploaded: self.update_time.as_deref().and_then(parse_instant),
			url: Some(chapter_page_url(&id)),
			..Default::default()
		}
	}
}

/// Chapters arrive newest-first, so unnumbered titles fall back to their
/// position counted from the oldest chapter.
pub fn number_chapters(chapters: &mut [Chapter]) {
	let count = chapters.len();
	for (index, chapter) in chapters.iter_mut().enumerate() {
		if chapter.chapter_number.is_none() {
			chapter.chapter_number = Some((count - index) as f32);
		}
	}
}

fn parse_chapter_number(title: &str) -> Option<f32> {
	let trimmed = title.trim();
	let rest = trimmed.strip_prefix('第').unwrap_or(trimmed);
	let digits: String = rest
		.chars()
		.take_while(|c| c.is_ascii_digit() || *c == '.')
		.collect();
	let digits = digits.trim_end_matches('.');
	if digits.is_empty() {
		return None;
	}
	digits.parse::<f32>().ok()
}

/// `updateTime` is an ISO-8601 instant; try the shapes seen in the wild.
fn parse_instant(value: &str) -> Option<i64> {
	const FORMATS: &[&str] = &[
		"yyyy-MM-dd'T'HH:mm:ss.SSSXXX",
		"yyyy-MM-dd'T'HH:mm:ssXXX",
		"yyyy-MM-dd'T'HH:mm:ss.SSSZ",
		"yyyy-MM-dd'T'HH:mm:ssZ",
		"yyyy-MM-dd'T'HH:mm:ss",
		"yyyy-MM-dd HH:mm:ss",
	];
	for format in FORMATS {
		if let Some(timestamp) = parse_date(value, *format) {
			return Some(timestamp);
		}
	}
	None
}
