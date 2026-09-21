use aidoku::{
	Chapter, Manga, MangaStatus,
	alloc::{String, Vec, format, string::ToString as _, vec},
	imports::std::parse_date,
};
use serde::{Deserialize, Serialize};

use crate::net::HOME_URL;

const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss.SSSSSSXXX";

impl Comic {
	pub fn into_manga(self) -> Option<Manga> {
		let mut manga = Manga {
			key: self.id?.to_string(),
			..Default::default()
		};
		self.into_details(&mut manga);

		Some(manga)
	}

	pub fn into_details(self, manga: &mut Manga) {
		if let Some(title) = self.title {
			manga.title = title;
		}
		manga.cover = self.cover_url;
		manga.authors = self.authors.filter(|authors| !authors.is_empty());
		manga.description = self.summary.map(|summary| summary.trim().to_string());

		let mut tags: Vec<String> = self
			.tags
			.unwrap_or_default()
			.into_iter()
			.map(|tag| tag.name)
			.collect();
		if let Some(category) = self.categories {
			tags.push(category.name);
		}
		if let Some(region) = region(self.region.as_deref()) {
			tags.push(region.into());
		}
		manga.tags = Some(tags).filter(|tags| !tags.is_empty());

		manga.status = match self.is_finished {
			Some(true) => MangaStatus::Completed,
			Some(false) => MangaStatus::Ongoing,
			None => MangaStatus::Unknown,
		};

		if let Some(id) = self.id {
			manga.url = Some(format!("{HOME_URL}/comic/{id}"));
		}
	}
}

fn region(region: Option<&str>) -> Option<&'static str> {
	match region? {
		"jp" => Some("日漫"),
		"kr" => Some("韩漫"),
		"us" => Some("美漫"),
		_ => None,
	}
}

impl ComicChapter {
	fn into_chapter(self, comic_id: &str) -> Chapter {
		// The page list request needs the chapter id and its page count, and the
		// comic id comes from the manga it belongs to.
		let key = format!("{}/{}", self.id, self.size);

		Chapter {
			key,
			title: Some(self.title),
			date_uploaded: parse_date(&self.updated_at, DATE_FORMAT),
			scanlators: scanlators(&self.category),
			url: Some(format!("{HOME_URL}/comic/{comic_id}/chapter-{}", self.idx)),
			..Default::default()
		}
	}
}

fn scanlators(category: &str) -> Option<Vec<String>> {
	match category {
		"normal" => Some(vec!["连载".into()]),
		"special" => Some(vec!["特典番外".into()]),
		"volume" => Some(vec!["单行本".into()]),
		_ => None,
	}
}

pub fn chapters(mut chapters: Vec<ComicChapter>, comic_id: &str) -> Vec<Chapter> {
	// Single volumes share the index range of the regular chapters, so they are
	// listed after them.
	chapters.sort_by_key(|chapter| (chapter.category == "volume", chapter.idx));
	chapters
		.into_iter()
		.map(|chapter| chapter.into_chapter(comic_id))
		.collect()
}

#[derive(Deserialize)]
pub struct Comic {
	pub id: Option<i32>,
	pub title: Option<String>,
	pub summary: Option<String>,
	#[serde(rename = "cover_url")]
	pub cover_url: Option<String>,
	#[serde(rename = "is_finished")]
	pub is_finished: Option<bool>,
	pub authors: Option<Vec<String>>,
	pub region: Option<String>,
	pub tags: Option<Vec<Tag>>,
	pub categories: Option<Tag>,
	pub chapters: Option<Vec<ComicChapter>>,
}

#[derive(Deserialize)]
pub struct Tag {
	pub name: String,
}

#[derive(Deserialize)]
pub struct ComicChapter {
	pub id: i32,
	pub idx: i32,
	pub title: String,
	pub category: String,
	#[serde(rename = "updated_at")]
	pub updated_at: String,
	#[serde(rename = "image_count")]
	pub size: i32,
}

// Only used to resolve a chapter url, which doesn't carry the chapter id.
#[derive(Deserialize)]
pub struct ChapterRow {
	pub id: i32,
	#[serde(rename = "image_count")]
	pub size: i32,
}

#[derive(Deserialize)]
pub struct PagesResult {
	pub urls: Vec<PageUrl>,
}

#[derive(Deserialize)]
pub struct PageUrl {
	pub url: String,
}

#[derive(Deserialize)]
pub struct LoginResult {
	#[serde(rename = "access_token")]
	pub access_token: String,
}

#[derive(Serialize)]
pub struct SearchBody<'a> {
	pub search_term: &'a str,
	pub items_per_page: String,
	pub page_number: String,
}

#[derive(Serialize)]
pub struct LoginBody<'a> {
	pub email: &'a str,
	pub password: &'a str,
	pub gotrue_meta_security: MetaSecurity,
}

#[derive(Serialize)]
pub struct MetaSecurity {}

#[derive(Serialize)]
pub struct PageListBody {
	pub comic_id: String,
	pub chapter_id: String,
	pub pages: Vec<String>,
	pub timestamp: i64,
	pub signature: String,
	pub client_diag: ClientDiag,
}

#[derive(Serialize)]
pub struct ClientDiag {
	pub native_status: String,
	pub native_fingerprint_prefix: String,
	pub canonical_payload_sha256: String,
	pub app_version: String,
	pub app_version_code: i32,
}
