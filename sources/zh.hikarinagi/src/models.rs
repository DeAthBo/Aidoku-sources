use aidoku::{
	Manga, MangaStatus,
	alloc::{String, Vec, format, string::ToString, vec},
};
use serde::Deserialize;
use serde_json::Value;

use crate::{BASE_URL, IMAGE_BASE_URL};

#[derive(Deserialize)]
pub struct BrowseResponse {
	pub list: BrowseList,
}

#[derive(Deserialize)]
pub struct BrowseList {
	#[serde(default)]
	pub items: Vec<MangaItem>,
	pub meta: Meta,
}

#[derive(Deserialize)]
pub struct Meta {
	pub page: i32,
	#[serde(rename = "total_pages")]
	pub total_pages: i32,
}

#[derive(Deserialize)]
pub struct MangaItem {
	pub id: i64,
	pub name: String,
	#[serde(rename = "name_cn")]
	pub name_cn: Option<String>,
	#[serde(default)]
	pub covers: Vec<Cover>,
	#[serde(rename = "serial_status")]
	pub serial_status: Option<String>,
	#[serde(rename = "latest_chapter_at")]
	pub latest_chapter_at: Option<String>,
	pub summary: Option<String>,
}

#[derive(Deserialize)]
pub struct Cover {
	pub media: Media,
}

#[derive(Deserialize)]
pub struct Media {
	pub src: String,
}

impl MangaItem {
	/// Mirrors `MangaItem.toSManga` in the upstream extension. The people/tags
	/// arguments are only available on the detail endpoint.
	pub fn into_manga(
		self,
		author: Option<String>,
		artist: Option<String>,
		tags: Option<Vec<String>>,
	) -> Manga {
		let key = self.id.to_string();
		let url = Some(format!("{BASE_URL}/mangas/{key}"));

		let cover = self.covers.into_iter().next().map(|cover| {
			let src = cover.media.src;
			if src.starts_with("http") {
				src
			} else {
				format!("{IMAGE_BASE_URL}/{src}")
			}
		});

		let status = match self.serial_status.as_deref() {
			Some("SERIALIZING") => MangaStatus::Ongoing,
			Some("FINISHED") => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};

		Manga {
			key,
			title: self.name_cn.unwrap_or(self.name),
			cover,
			authors: author.map(|name| vec![name]),
			artists: artist.map(|name| vec![name]),
			description: self.summary,
			url,
			tags,
			status,
			..Default::default()
		}
	}
}

#[derive(Deserialize)]
pub struct MangaData {
	pub manga: MangaItem,
	#[serde(default)]
	pub chapters: Vec<ChapterItem>,
	#[serde(default)]
	pub people: Vec<PersonEntry>,
	#[serde(default)]
	pub tags: Vec<TagEntry>,
}

#[derive(Deserialize)]
pub struct PersonEntry {
	pub role: String,
	pub person: Person,
}

#[derive(Deserialize)]
pub struct Person {
	pub name: String,
	#[serde(rename = "trans_name")]
	pub trans_name: Option<String>,
}

#[derive(Deserialize)]
pub struct TagEntry {
	pub tag: Tag,
}

#[derive(Deserialize)]
pub struct Tag {
	pub name: String,
}

/// The upstream `people()` map keeps the last entry for a repeated role.
pub fn find_person(people: &[PersonEntry], role: &str) -> Option<String> {
	let mut value = None;
	for entry in people {
		if entry.role == role {
			value = Some(
				entry
					.person
					.trans_name
					.clone()
					.unwrap_or_else(|| entry.person.name.clone()),
			);
		}
	}
	value
}

#[derive(Deserialize)]
pub struct ChapterItem {
	pub id: i64,
	pub title: Option<String>,
	#[serde(rename = "chapter_number")]
	pub number: Option<String>,
	#[serde(rename = "page_count")]
	pub size: Option<i32>,
}

#[derive(Deserialize)]
pub struct ReaderResponse {
	pub manifest: Manifest,
}

#[derive(Deserialize)]
pub struct Manifest {
	#[serde(default)]
	pub pages: Vec<ManifestPage>,
}

#[derive(Deserialize)]
pub struct ManifestPage {
	/// The upstream reads this with `getString("id")`; accept a number too in
	/// case the API ever serialises it as one.
	#[serde(default)]
	pub id: Value,
	#[serde(rename = "mime_type", default)]
	pub mime_type: String,
}

pub fn value_to_string(value: Value) -> Option<String> {
	match value {
		Value::String(s) => Some(s),
		Value::Number(n) => Some(n.to_string()),
		_ => None,
	}
}
