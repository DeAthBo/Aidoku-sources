use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::std::parse_date,
};
use serde_json::Value;

use crate::net::BASE_URL;

fn as_string(value: &Value) -> Option<String> {
	value.as_str().map(String::from)
}

fn as_i64(value: &Value) -> Option<i64> {
	value.as_i64().or_else(|| value.as_f64().map(|f| f as i64))
}

fn manga_from_value(value: &Value) -> Option<Manga> {
	let id = as_i64(value.get("id")?)?;
	let title = as_string(value.get("title")?).unwrap_or_default();
	if title.is_empty() {
		return None;
	}
	Some(Manga {
		key: id.to_string(),
		title,
		cover: value.get("vertical_image_url").and_then(as_string),
		authors: value
			.get("author_name")
			.and_then(as_string)
			.map(|author| vec![author]),
		description: value.get("description").and_then(as_string),
		url: Some(format!("{BASE_URL}/web/topic/{id}")),
		..Default::default()
	})
}

pub fn web_list_result(value: &Value) -> MangaPageResult {
	let data0 = value.get("data").and_then(|data| data.get(0));
	let entries: Vec<Manga> = data0
		.and_then(|data| data.get("dataList"))
		.and_then(Value::as_array)
		.map(|list| list.iter().filter_map(manga_from_value).collect())
		.unwrap_or_default();

	let has_next_page = match (
		data0.and_then(|d| d.get("page")).and_then(as_i64),
		data0.and_then(|d| d.get("count")).and_then(as_i64),
		data0.and_then(|d| d.get("total")).and_then(as_i64),
	) {
		(Some(page), Some(count), Some(total)) => page.saturating_mul(count) < total,
		_ => entries.len() >= 20,
	};

	MangaPageResult {
		entries,
		has_next_page,
	}
}

pub fn api_search_result(value: &Value) -> MangaPageResult {
	let data = value.get("data");
	let entries: Vec<Manga> = data
		.and_then(|d| d.get("hit"))
		.and_then(Value::as_array)
		.map(|list| list.iter().filter_map(manga_from_value).collect())
		.unwrap_or_default();
	let has_next_page = data
		.and_then(|d| d.get("since"))
		.and_then(as_i64)
		.is_some_and(|since| since >= 0);
	MangaPageResult {
		entries,
		has_next_page,
	}
}

pub fn update_details(value: &Value, manga: &mut Manga) {
	let data0 = value.get("data").and_then(|data| data.get(0));
	let info = data0.and_then(|data| data.get("topicInfo")).or_else(|| {
		data0
			.and_then(|data| data.get("res"))
			.and_then(|res| res.get("data"))
			.and_then(|data| data.get("topic_info"))
	});
	if let Some(info) = info {
		if let Some(title) = info.get("title").and_then(as_string) {
			manga.title = title;
		}
		manga.cover = info.get("vertical_image_url").and_then(as_string);
		manga.description = info.get("description").and_then(as_string);
		manga.status = match info.get("update_status").and_then(as_string).as_deref() {
			Some("连载中") => MangaStatus::Ongoing,
			Some("已完结") => MangaStatus::Completed,
			_ => MangaStatus::Unknown,
		};
		manga.authors = info
			.get("user")
			.and_then(|user| user.get("nickname"))
			.and_then(as_string)
			.or_else(|| info.get("author_name").and_then(as_string))
			.map(|author| vec![author]);
	}
	manga.url = Some(format!("{BASE_URL}/web/topic/{}", manga.key));
}

pub fn chapters(value: &Value) -> Vec<Chapter> {
	let data0 = value.get("data").and_then(|data| data.get(0));
	let comics = data0.and_then(|data| data.get("comics")).or_else(|| {
		data0
			.and_then(|data| data.get("res"))
			.and_then(|res| res.get("data"))
			.and_then(|data| data.get("topic_info"))
			.and_then(|info| info.get("comics"))
	});
	let mut chapters: Vec<Chapter> = comics
		.and_then(Value::as_array)
		.map(|list| list.iter().filter_map(chapter_from_value).collect())
		.unwrap_or_default();
	// The API returns oldest first; Aidoku expects newest first.
	chapters.reverse();
	let count = chapters.len();
	for (index, chapter) in chapters.iter_mut().enumerate() {
		chapter.chapter_number.get_or_insert((count - index) as f32);
	}
	chapters
}

fn chapter_from_value(value: &Value) -> Option<Chapter> {
	let id = as_i64(value.get("id")?)?;
	let title = as_string(value.get("title")?).unwrap_or_default();
	let date_uploaded = value
		.get("created_at")
		.and_then(as_string)
		.and_then(|date| parse_date(&date, "yy-MM-dd"));
	let locked = value
		.get("locked")
		.and_then(Value::as_bool)
		.unwrap_or(false)
		|| value
			.get("is_pay_comic")
			.and_then(Value::as_bool)
			.unwrap_or(false);
	Some(Chapter {
		key: format!("/web/comic/{id}"),
		title: Some(title.clone()),
		chapter_number: leading_number(&title),
		date_uploaded,
		locked,
		url: Some(format!("{BASE_URL}/web/comic/{id}")),
		..Default::default()
	})
}

fn leading_number(title: &str) -> Option<f32> {
	title
		.trim()
		.trim_start_matches('第')
		.chars()
		.take_while(|c| c.is_ascii_digit())
		.collect::<String>()
		.parse()
		.ok()
}

pub fn pages(value: &Value) -> Vec<Page> {
	let data0 = value.get("data").and_then(|data| data.get(0));
	let images = data0
		.and_then(|data| data.get("res"))
		.and_then(|res| res.get("data"))
		.and_then(|data| data.get("comic_info"))
		.and_then(|info| info.get("comic_images"))
		.or_else(|| {
			data0
				.and_then(|data| data.get("comicInfo"))
				.and_then(|info| info.get("comic_images"))
		});
	images
		.and_then(Value::as_array)
		.map(|list| list.iter().filter_map(page_from_value).collect())
		.unwrap_or_default()
}

fn page_from_value(value: &Value) -> Option<Page> {
	let url = value
		.get("url1280")
		.and_then(as_string)
		.filter(|url| !url.is_empty())
		.or_else(|| value.get("url").and_then(as_string))?;
	Some(Page {
		content: PageContent::url(url),
		..Default::default()
	})
}

/// Fallback for the `/v2/comic/<cid>` endpoint when the Nuxt page is unavailable.
pub fn api_comic_pages(value: &Value) -> Vec<Page> {
	value
		.get("data")
		.and_then(|data| data.get("images"))
		.and_then(Value::as_array)
		.map(|list| {
			list.iter()
				.filter_map(as_string)
				.map(|url| Page {
					content: PageContent::url(url),
					..Default::default()
				})
				.collect()
		})
		.unwrap_or_default()
}
