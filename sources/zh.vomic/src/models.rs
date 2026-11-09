use aidoku::{
	Chapter, Manga, MangaStatus, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::std::parse_date,
	prelude::*,
};
use serde::{Deserialize, Serialize};

use crate::net::{chapter_page_url, manga_page_url};

#[derive(Deserialize)]
pub struct ApiResult<T> {
	pub data: T,
	pub code: Option<i32>,
}

impl<T> ApiResult<T> {
	/// The upstream `result` getter throws unless `code == 200`.
	pub fn unpack(self) -> Result<T> {
		if self.code != Some(200) {
			bail!("vomic API error (code: {:?})", self.code);
		}
		Ok(self.data)
	}
}

#[derive(Deserialize)]
pub struct Token {
	pub token: String,
}

#[derive(Serialize)]
pub struct LoginPayload {
	pub identity: String,
	pub password: String,
}

#[derive(Deserialize)]
pub struct MangaDto {
	pub id: i32,
	pub name: String,
	pub intro: Option<String>,
	pub cover: Option<String>,
	pub author: Option<String>,
	pub process: Option<String>,
	pub tags: Option<String>,
	#[serde(default)]
	pub cat: Vec<CategoryDto>,
	#[serde(default)]
	pub tag: Vec<MangaTag>,
	#[serde(rename = "likes_num", default)]
	pub likes: i32,
	#[serde(rename = "view_num", default)]
	pub views: i32,
	#[serde(rename = "chapter_list", default)]
	pub chapter_list: Vec<ChapterGroup>,
}

#[derive(Deserialize)]
pub struct CategoryDto {
	pub id: i32,
	pub name: String,
}

#[derive(Deserialize)]
pub struct MangaTag {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ChapterGroup {
	#[serde(default)]
	pub values: Vec<ChapterDto>,
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub id: i32,
	pub name: String,
	#[serde(rename = "car_id")]
	pub manga_id: i32,
	#[serde(rename = "group_name")]
	pub group_name: Option<String>,
	#[serde(rename = "updated_at")]
	pub updated_at: Option<String>,
	#[serde(rename = "created_at")]
	pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub struct Pages {
	#[serde(rename = "img_list", default)]
	pub images: Vec<ImageDto>,
}

#[derive(Deserialize)]
pub struct ImageDto {
	pub url: Option<String>,
}

impl MangaDto {
	pub fn into_manga(self) -> Manga {
		let tags = build_tags(&self);
		let description = build_description(&self);
		let status = manga_status(self.process.as_deref());
		let id = self.id.to_string();
		Manga {
			key: id.clone(),
			title: self.name,
			cover: self.cover,
			authors: self.author.map(|author| vec![author]),
			tags,
			status,
			description: Some(description),
			url: Some(manga_page_url(&id)),
			..Default::default()
		}
	}

	pub fn apply_details(&self, manga: &mut Manga) {
		let id = self.id.to_string();
		manga.key = id.clone();
		manga.title = self.name.clone();
		manga.cover = self.cover.clone();
		manga.authors = self.author.clone().map(|author| vec![author]);
		manga.tags = build_tags(self);
		manga.status = manga_status(self.process.as_deref());
		manga.description = Some(build_description(self));
		manga.url = Some(manga_page_url(&id));
	}
}

fn manga_status(process: Option<&str>) -> MangaStatus {
	match process {
		Some("连载中") => MangaStatus::Ongoing,
		Some("已完结") => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	}
}

fn build_tags(manga: &MangaDto) -> Option<Vec<String>> {
	let mut tags: Vec<String> = Vec::new();
	for category in &manga.cat {
		push_distinct(&mut tags, category.name.clone());
	}
	for tag in &manga.tag {
		push_distinct(&mut tags, tag.name.clone());
	}
	if let Some(raw) = manga.tags.as_deref() {
		for part in raw.split(' ') {
			let part = part.trim();
			if !part.is_empty() {
				push_distinct(&mut tags, part.to_string());
			}
		}
	}
	if tags.is_empty() { None } else { Some(tags) }
}

fn push_distinct(tags: &mut Vec<String>, value: String) {
	if !tags.iter().any(|tag| tag == &value) {
		tags.push(value);
	}
}

fn build_description(manga: &MangaDto) -> String {
	let mut description = format!(
		" \u{1F441}\u{FE0F} {} - \u{2764}\u{FE0F} {}",
		thousands(manga.views),
		thousands(manga.likes)
	);
	if let Some(intro) = manga
		.intro
		.as_deref()
		.filter(|intro| !intro.trim().is_empty())
	{
		description.push_str("\n\n");
		description.push_str(intro);
	}
	description
}

/// `%,d`-style grouping of the view/like counters.
fn thousands(value: i32) -> String {
	let digits = value.to_string();
	let mut result = String::with_capacity(digits.len() + digits.len() / 3);
	for (index, character) in digits.chars().enumerate() {
		if index > 0 && (digits.len() - index).is_multiple_of(3) {
			result.push(',');
		}
		result.push(character);
	}
	result
}

impl ChapterDto {
	pub fn to_chapter(&self) -> Chapter {
		let manga_id = self.manga_id.to_string();
		let id = self.id.to_string();
		let date = self.created_at.as_deref().or(self.updated_at.as_deref());
		Chapter {
			key: format!("{manga_id}/{id}"),
			title: Some(self.name.clone()),
			chapter_number: parse_chapter_number(&self.name),
			date_uploaded: date.and_then(|value| parse_date(value, "yyyy-MM-dd HH:mm:ss")),
			scanlators: self.group_name.clone().map(|group| vec![group]),
			url: Some(chapter_page_url(&manga_id, &id)),
			..Default::default()
		}
	}
}

/// The API already returns chapters newest-first; only fill in missing numbers.
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
		.take_while(|character| character.is_ascii_digit() || *character == '.')
		.collect();
	let digits = digits.trim_end_matches('.');
	if digits.is_empty() {
		return None;
	}
	digits.parse::<f32>().ok()
}
