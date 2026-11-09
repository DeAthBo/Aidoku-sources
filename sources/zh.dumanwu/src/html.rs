use crate::{crypto, models, net};
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result,
	alloc::{String, Vec, string::ToString as _},
	imports::html::{Document, Element},
	prelude::*,
};

pub fn parse_ranking(document: &Document) -> MangaPageResult {
	let entries = document
		.select(".likedata")
		.map(|list| list.filter_map(ranking_manga).collect())
		.unwrap_or_default();
	MangaPageResult {
		entries,
		has_next_page: false,
	}
}

fn ranking_manga(element: Element) -> Option<Manga> {
	let href = element.select_first("a")?.attr("href")?;
	let key = trim_slashes(&href);
	Some(Manga {
		key: key.to_string(),
		title: element.select_first(".le-t")?.text()?,
		authors: element
			.select_first(".likeinfo > p")
			.and_then(|p| p.text())
			.map(|text| {
				text.strip_prefix("作者：")
					.unwrap_or(text.as_str())
					.trim()
					.to_string()
			})
			.filter(|author| !author.is_empty())
			.map(single),
		description: element.select_first(".le-j").and_then(|e| e.text()),
		cover: element
			.select_first("img")
			.and_then(|img| img.attr("data-src"))
			.map(|url| net::absolute(&url)),
		url: Some(format!("{}/{key}/", net::BASE_URL)),
		..Default::default()
	})
}

pub fn parse_search(document: &Document) -> MangaPageResult {
	let entries = document
		.select(".item-data > div")
		.map(|list| list.filter_map(search_manga).collect())
		.unwrap_or_default();
	MangaPageResult {
		entries,
		has_next_page: false,
	}
}

fn search_manga(element: Element) -> Option<Manga> {
	let href = element.select_first("a")?.attr("href")?;
	let key = trim_slashes(&href);
	Some(Manga {
		key: key.to_string(),
		title: element.select_first(".e-title, .title")?.text()?,
		authors: element
			.select_first(".tip")
			.and_then(|e| e.text())
			.filter(|text| !text.is_empty())
			.map(single),
		cover: element
			.select_first("img")
			.and_then(|img| img.attr("data-src"))
			.map(|url| net::absolute(&url)),
		url: Some(format!("{}/{key}/", net::BASE_URL)),
		..Default::default()
	})
}

pub fn update_details(document: &Document, manga: &mut Manga) {
	let Some(info) = document.select_first(".comicInfo") else {
		return;
	};
	manga.cover = info
		.select_first("img")
		.and_then(|img| img.attr("data-src"))
		.map(|url| net::absolute(&url));

	let Some(container) = info.select_first(".detinfo") else {
		return;
	};
	if let Some(title) = container.select_first("h1").and_then(|h| h.text()) {
		manga.title = title;
	}

	let mut updated = String::new();
	if let Some(spans) = container.select("span") {
		for span in spans {
			let text = normalize_ws(&span.own_text().unwrap_or_default());
			if text.starts_with("更新时间") {
				updated = format!("{text}\n\n");
			} else if let Some((key, value)) =
				text.split_once('：').or_else(|| text.split_once(':'))
			{
				let key = key.trim();
				let value = value.trim();
				if key.contains('作') && key.contains('者') && !value.is_empty() {
					manga.authors = Some(single(value.to_string()));
				} else if key.contains('标') && key.contains('签') {
					manga.tags = Some(
						value
							.split(' ')
							.map(|tag| tag.trim().to_string())
							.filter(|tag| !tag.is_empty())
							.collect(),
					);
				} else if key.contains('状') && key.contains('态') {
					manga.status = match value {
						"连载中" => MangaStatus::Ongoing,
						"已完结" => MangaStatus::Completed,
						_ => MangaStatus::Unknown,
					};
				}
			}
		}
	}

	let description = container
		.select_first(".content")
		.and_then(|e| e.text())
		.unwrap_or_default();
	manga.description = Some(format!("{updated}{description}"));
	manga.url = Some(format!(
		"{}/{}/",
		net::BASE_URL,
		manga.key.trim_matches('/')
	));
}

pub fn parse_chapters(document: &Document, manga_key: &str) -> Result<Vec<Chapter>> {
	let mut chapters: Vec<Chapter> = Vec::new();
	let mut has_more = false;
	if let Some(container) = document.select_first(".chapterlistload") {
		if let Some(links) = container.select("a[href]") {
			for link in links {
				if let Some(chapter) = chapter_from_link(&link) {
					chapters.push(chapter);
				}
			}
		}
		has_more = container.select_first(".chaplist-more").is_some();
	}

	if has_more {
		let body = net::form_pair("id", manga_key);
		let url = format!("{}/morechapter", net::DESKTOP_URL);
		let referer = format!("{}/{manga_key}/", net::DESKTOP_URL);
		let response: models::ResponseDto =
			net::post_form(&url, body, &referer, true)?.json_owned()?;
		for item in response.data {
			let number = leading_number(&item.chaptername);
			let key = format!("{manga_key}/{}", item.chapterid);
			let url = Some(format!("{}/{key}.html", net::BASE_URL));
			chapters.push(Chapter {
				key,
				title: Some(item.chaptername),
				chapter_number: number,
				url,
				..Default::default()
			});
		}
	}

	let count = chapters.len();
	for (index, chapter) in chapters.iter_mut().enumerate() {
		chapter.chapter_number.get_or_insert((count - index) as f32);
	}
	Ok(chapters)
}

fn chapter_from_link(link: &Element) -> Option<Chapter> {
	let href = link.attr("href")?;
	let key = chapter_key(&href)?;
	let title = link.text()?;
	let number = leading_number(&title);
	let url = Some(format!("{}/{key}.html", net::BASE_URL));
	Some(Chapter {
		key,
		title: Some(title),
		chapter_number: number,
		url,
		..Default::default()
	})
}

pub fn pages(chapter_key: &str) -> Result<Vec<Page>> {
	let url = format!("{}/{chapter_key}.html", net::BASE_URL);
	let document = net::get(&url)?.html()?;
	let index: usize = document
		.select_first(".readerContainer")
		.and_then(|e| e.attr("data-id"))
		.and_then(|id| id.trim().parse().ok())
		.ok_or_else(|| error!("Missing readerContainer data-id"))?;
	let script = document
		.select("script")
		.and_then(|list| {
			list.filter_map(|e| e.data())
				.find(|data| data.contains("eval") && data.contains("}('"))
		})
		.ok_or_else(|| error!("Missing packed script"))?;
	let unpacked = crypto::unpack(&script).ok_or_else(|| error!("Failed to unpack script"))?;
	let payload = unpacked.trim().trim_end_matches(';');
	let encoded = payload
		.strip_prefix("var __c0rst96=\"")
		.and_then(|s| s.strip_suffix('"'))
		.ok_or_else(|| error!("Unexpected unpacked payload"))?;
	let json = crypto::decrypt(encoded, index).ok_or_else(|| error!("Failed to decrypt pages"))?;
	let urls: Vec<String> =
		serde_json::from_str(&json).map_err(|_| error!("Failed to parse page urls"))?;
	Ok(urls
		.into_iter()
		.map(|url| Page {
			content: PageContent::url(net::absolute(&url)),
			..Default::default()
		})
		.collect())
}

fn trim_slashes(value: &str) -> &str {
	value.trim_matches('/')
}

fn single(value: String) -> Vec<String> {
	Vec::from([value])
}

fn normalize_ws(text: &str) -> String {
	text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn chapter_key(href: &str) -> Option<String> {
	let path = if let Some(pos) = href.find("://") {
		let rest = &href[pos + 3..];
		match rest.find('/') {
			Some(i) => &rest[i + 1..],
			None => "",
		}
	} else {
		href.trim_start_matches('/')
	};
	let path = path
		.strip_suffix(".html")
		.unwrap_or(path)
		.trim_start_matches('/');
	if path.is_empty() {
		None
	} else {
		Some(path.to_string())
	}
}

fn leading_number(title: &str) -> Option<f32> {
	title
		.trim_start_matches('第')
		.chars()
		.take_while(|c| c.is_ascii_digit())
		.collect::<String>()
		.parse()
		.ok()
}
