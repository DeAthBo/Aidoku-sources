use crate::net;
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result,
	alloc::{String, Vec, string::ToString as _},
	helpers::uri::QueryParameters,
	imports::html::{Document, Element},
	prelude::*,
};

pub fn parse_manga_list(document: &Document) -> MangaPageResult {
	let entries = document
		.select("#browseSection a[href^=/comic/]")
		.map(|list| list.filter_map(manga_from_list).collect())
		.unwrap_or_default();
	MangaPageResult {
		entries,
		has_next_page: next_page_target(document).is_some(),
	}
}

fn manga_from_list(element: Element) -> Option<Manga> {
	let href = element.attr("href")?;
	let key = comic_slug(&href)?;
	Some(Manga {
		key: key.clone(),
		title: element.select_first(".comic-title")?.text()?,
		cover: element
			.select_first("img")
			.and_then(|img| img.attr("abs:src").or_else(|| img.attr("src")))
			.map(|url| net::absolute(&url)),
		url: Some(net::manga_url(&key)),
		..Default::default()
	})
}

pub fn next_page_target(document: &Document) -> Option<String> {
	let list = document.select("a[id^=MainContent_rptPager_lnkPage]")?;
	for element in list {
		if element.text().as_deref() == Some(">") {
			return element.attr("href").and_then(|href| postback_target(&href));
		}
	}
	None
}

pub fn post_back(document: &Document, target: &str, referer: &str) -> Result<Document> {
	let form = document
		.select_first("form#ctl01")
		.ok_or_else(|| error!("form#ctl01 not found"))?;
	let mut params = QueryParameters::new();
	params.push("__EVENTTARGET", Some(target));

	if let Some(inputs) = form.select("input[name]") {
		for input in inputs {
			let Some(name) = input.attr("name") else {
				continue;
			};
			let kind = input.attr("type").unwrap_or_default();
			match kind.as_str() {
				"submit" | "button" | "image" | "file" => {}
				"checkbox" | "radio" => {
					if input.has_attr("checked") {
						let value = input.attr("value").unwrap_or_default();
						params.push(name.as_str(), Some(value.as_str()));
					}
				}
				_ => {
					if name != "__EVENTTARGET" {
						let value = input.attr("value").unwrap_or_default();
						params.push(name.as_str(), Some(value.as_str()));
					}
				}
			}
		}
	}

	if let Some(selects) = form.select("select[name]") {
		for select in selects {
			let Some(name) = select.attr("name") else {
				continue;
			};
			let option = select
				.select_first("option[selected]")
				.or_else(|| select.select_first("option"));
			let value = option.and_then(|o| o.attr("value")).unwrap_or_default();
			params.push(name.as_str(), Some(value.as_str()));
		}
	}

	let action = form
		.attr("abs:action")
		.or_else(|| form.attr("action"))
		.filter(|url| !url.is_empty());
	let action = match action {
		Some(url) => net::absolute(&url),
		None => referer.to_string(),
	};
	Ok(net::post_form(&action, params.to_string(), referer)?.html()?)
}

fn postback_target(href: &str) -> Option<String> {
	let rest = href.split("__doPostBack('").nth(1)?;
	let target = rest.split('\'').next()?;
	if target.is_empty() {
		None
	} else {
		Some(target.to_string())
	}
}

pub fn comic_slug(href: &str) -> Option<String> {
	let path = if let Some(pos) = href.find("://") {
		let rest = &href[pos + 3..];
		match rest.find('/') {
			Some(index) => &rest[index + 1..],
			None => "",
		}
	} else {
		href.trim_start_matches('/')
	};
	let slug = path.strip_prefix("comic/")?;
	let slug = slug.split(['/', '?', '#']).next().unwrap_or("").trim();
	if slug.is_empty() {
		None
	} else {
		Some(slug.to_string())
	}
}

pub fn query_param(href: &str, name: &str) -> Option<String> {
	let query = href.split_once('?')?.1;
	for pair in query.split('&') {
		if let Some((key, value)) = pair.split_once('=')
			&& key == name
		{
			return Some(value.to_string());
		}
	}
	None
}

pub fn update_details(document: &Document, manga: &mut Manga) {
	if let Some(title) = document
		.select_first("#MainContent_lblTitle")
		.and_then(|element| element.text())
	{
		manga.title = title;
	}
	manga.authors = document
		.select_first("#MainContent_lblAuthor")
		.and_then(|element| element.text())
		.filter(|author| !author.is_empty())
		.map(|author| Vec::from([author]));
	manga.status = match document
		.select_first("#MainContent_lblStatus")
		.and_then(|element| element.text())
		.unwrap_or_default()
		.to_lowercase()
		.as_str()
	{
		"ongoing" => MangaStatus::Ongoing,
		"completed" => MangaStatus::Completed,
		"hiatus" => MangaStatus::Hiatus,
		_ => MangaStatus::Unknown,
	};
	manga.tags = document
		.select(".cd-tag")
		.map(|list| {
			list.filter_map(|element| element.text())
				.map(|tag| tag.trim().to_string())
				.filter(|tag| !tag.is_empty())
				.collect::<Vec<String>>()
		})
		.filter(|tags| !tags.is_empty());

	let mut description = String::new();
	if let Some(text) = document
		.select_first("#MainContent_lblDescription")
		.and_then(|element| element.text())
	{
		description.push_str(&text);
	}
	if let Some(other) = document
		.select_first("#MainContent_lblOtherName")
		.and_then(|element| element.text())
		.filter(|text| !text.trim().is_empty())
	{
		description.push_str("\n\nAlternative name: ");
		description.push_str(&other);
	}
	manga.description = if description.is_empty() {
		None
	} else {
		Some(description)
	};
	manga.cover = document
		.select_first("#MainContent_imgCover")
		.and_then(|element| element.attr("abs:src").or_else(|| element.attr("src")))
		.map(|url| net::absolute(&url));
	manga.url = Some(net::manga_url(manga.key.trim_matches('/')));
}

pub fn chapters(first: &Document, referer: &str) -> Result<Vec<Chapter>> {
	let mut all: Vec<Chapter> = Vec::new();
	append_chapters(first, &mut all);

	if let Some(list) =
		first.select("#MainContent_ctl12 a[href*=rptChapterRanges]:not(.btn-primary)")
	{
		for element in list {
			if let Some(href) = element.attr("href")
				&& let Some(target) = postback_target(&href)
				&& let Ok(document) = post_back(first, &target, referer)
			{
				append_chapters(&document, &mut all);
			}
		}
	}

	let mut seen: Vec<String> = Vec::new();
	let mut unique: Vec<Chapter> = Vec::new();
	for chapter in all {
		if !seen.contains(&chapter.key) {
			seen.push(chapter.key.clone());
			unique.push(chapter);
		}
	}
	unique.reverse();

	let count = unique.len();
	for (index, chapter) in unique.iter_mut().enumerate() {
		chapter.chapter_number.get_or_insert((count - index) as f32);
	}
	Ok(unique)
}

fn append_chapters(document: &Document, out: &mut Vec<Chapter>) {
	if let Some(list) = document.select("li.cd-chapter-item") {
		for item in list {
			let Some(link) = item.select_first("a.cd-chapter-link") else {
				continue;
			};
			let Some(href) = link.attr("href") else {
				continue;
			};
			let Some(key) = query_param(&href, "id") else {
				continue;
			};
			let text = link.text().unwrap_or_default();
			let (title, number) = chapter_title(&text);
			let date = item
				.select_first("span.small")
				.and_then(|element| element.text())
				.and_then(|text| parse_date(&text));
			let url = Some(net::chapter_url(&key));
			out.push(Chapter {
				key,
				title: Some(title),
				chapter_number: number,
				date_uploaded: date,
				url,
				..Default::default()
			});
		}
	}
}

fn chapter_title(text: &str) -> (String, Option<f32>) {
	let text = text.trim();
	let bytes = text.as_bytes();
	let mut index = 0;
	while index < bytes.len() && !bytes[index].is_ascii_digit() {
		index += 1;
	}
	if index < bytes.len() {
		let start = index;
		let mut dots = 0;
		while index < bytes.len()
			&& (bytes[index].is_ascii_digit() || (bytes[index] == b'.' && dots == 0))
		{
			if bytes[index] == b'.' {
				dots += 1;
			}
			index += 1;
		}
		let number = &text[start..index];
		let rest = text[index..].trim_start();
		if let Some(after) = rest.strip_prefix(':') {
			return (
				format!("Chapter {number}: {}", after.trim()),
				number.parse().ok(),
			);
		}
	}
	(text.to_string(), None)
}

fn parse_date(text: &str) -> Option<i64> {
	let mut parts = text.trim().split('/');
	let day: i64 = parts.next()?.trim().parse().ok()?;
	let month: i64 = parts.next()?.trim().parse().ok()?;
	let year: i64 = parts.next()?.trim().parse().ok()?;
	if !(1..=31).contains(&day) || !(1..=12).contains(&month) || year < 1970 {
		return None;
	}
	Some(days_from_civil(year, month, day) * 86_400)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
	let y = year - if month <= 2 { 1 } else { 0 };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400;
	let mp = month + if month > 2 { -3 } else { 9 };
	let doy = (153 * mp + 2) / 5 + day - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146_097 + doe - 719_468
}

pub fn pages(document: &Document) -> Vec<Page> {
	document
		.select("img.chapter-image")
		.map(|list| {
			list.filter_map(|image| {
				let url = image
					.attr("abs:data-src")
					.filter(|url| !url.is_empty())
					.or_else(|| image.attr("data-src").filter(|url| !url.is_empty()))
					.or_else(|| image.attr("abs:src"))
					.or_else(|| image.attr("src"))?;
				Some(Page {
					content: PageContent::url(net::absolute(&url)),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default()
}
