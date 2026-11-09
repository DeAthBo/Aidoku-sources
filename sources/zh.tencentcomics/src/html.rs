use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::html::{Document, Element},
};

use crate::net::BASE_URL;

pub fn popular_page_result(doc: &Document) -> Result<MangaPageResult> {
	let entries: Vec<Manga> = doc
		.select("ul.ret-search-list.clearfix > li")
		.map(|elements| elements.filter_map(manga_from_popular).collect())
		.unwrap_or_default();
	// The site always fills a full page even when fewer results exist.
	let has_next_page = entries.len() == 12;
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

pub fn search_page_result(doc: &Document) -> Result<MangaPageResult> {
	let entries: Vec<Manga> = doc
		.select("ul > li.comic-item > a")
		.map(|elements| elements.filter_map(manga_from_search).collect())
		.unwrap_or_default();
	let has_next_page = entries.len() == 10;
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

fn manga_from_popular(element: Element) -> Option<Manga> {
	let anchor = element.select_first("div > a")?;
	let href = anchor.attr("href")?;
	let key = href
		.split_once("/Comic/comicInfo/")
		.map_or(href.as_str(), |(_, rest)| rest)
		.to_string();
	let title = anchor.attr("title")?.trim().to_string();

	Some(Manga {
		key: key.clone(),
		title,
		cover: element
			.select_first("div > a > img")
			.and_then(|img| img.attr("data-original")),
		authors: element
			.select_first("div > p.ret-works-author")
			.and_then(|p| p.text())
			.map(|author| vec![author]),
		description: element
			.select_first("div > p.ret-works-decs")
			.and_then(|p| p.text()),
		url: Some(format!("{BASE_URL}/comic/index/{key}")),
		..Default::default()
	})
}

fn manga_from_search(anchor: Element) -> Option<Manga> {
	let href = anchor.attr("href")?;
	let key = href
		.strip_prefix("/comic/index/")
		.unwrap_or(href.as_str())
		.to_string();

	let tags = anchor
		.select_first("div > small.comic-tag")
		.and_then(|e| e.text())
		.map(|text| {
			text.split(' ')
				.filter(|tag| !tag.is_empty())
				.map(String::from)
				.collect::<Vec<String>>()
		});

	Some(Manga {
		key: key.clone(),
		title: anchor
			.select_first("div > strong")
			.and_then(|e| e.text())
			.unwrap_or_default(),
		cover: anchor
			.select_first("div > img")
			.and_then(|img| img.attr("src")),
		description: anchor
			.select_first("div > small.comic-desc")
			.and_then(|e| e.text()),
		tags,
		url: Some(format!("{BASE_URL}/comic/index/{key}")),
		..Default::default()
	})
}

pub fn update_details(doc: &Document, manga: &mut Manga) {
	manga.cover = doc
		.select_first("div.works-cover.ui-left > a > img")
		.and_then(|img| img.attr("src"));
	if let Some(title) = doc
		.select_first(".works-intro-title > strong")
		.and_then(|e| e.text())
	{
		manga.title = title;
	}
	manga.description = doc
		.select_first("p.works-intro-short")
		.and_then(|e| e.text());
	manga.authors = doc
		.select_first("p.works-intro-digi > span > em")
		.and_then(|e| e.text())
		.map(|author| vec![author]);
	manga.status = match doc
		.select_first("label.works-intro-status")
		.and_then(|e| e.text())
		.as_deref()
	{
		Some("连载中") | Some("連載中") => MangaStatus::Ongoing,
		Some("已完结") | Some("已完結") => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	manga.url = Some(format!("{BASE_URL}/comic/index/{}", manga.key));
}

pub fn chapters(doc: &Document) -> Vec<Chapter> {
	let mut chapters: Vec<Chapter> = doc
		.select(".chapter-page-all .works-chapter-item")
		.map(|elements| elements.filter_map(chapter_from_element).collect())
		.unwrap_or_default();
	// The site lists oldest first; Aidoku expects newest first.
	chapters.reverse();
	let count = chapters.len();
	for (index, chapter) in chapters.iter_mut().enumerate() {
		chapter.chapter_number.get_or_insert((count - index) as f32);
	}
	chapters
}

fn chapter_from_element(element: Element) -> Option<Chapter> {
	let anchor = element.select_first("a")?;
	let url = anchor.attr("abs:href")?;
	let locked = element.select_first(".ui-icon-pay").is_some();
	let name = element.text().unwrap_or_default();
	let title = if locked {
		format!("🔒 {name}")
	} else {
		name.clone()
	};

	Some(Chapter {
		key: url.clone(),
		title: Some(title),
		chapter_number: leading_number(&name),
		url: Some(url),
		locked,
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
