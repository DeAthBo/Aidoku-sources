use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::html::{Document, ElementList},
};

/// Upstream stores keys with `setUrlWithoutDomain`, i.e. the path (and query)
/// of the absolute link, so they work against every mirror.
pub fn key_from_url(url: &str) -> String {
	let rest = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))
		.unwrap_or(url);
	match rest.find('/') {
		Some(index) => rest[index..].into(),
		None => String::from("/"),
	}
}

fn joined_text(elements: Option<ElementList>) -> String {
	match elements {
		Some(list) => list
			.filter_map(|element| element.text())
			.collect::<Vec<String>>()
			.join(", "),
		None => String::new(),
	}
}

fn text_list(elements: Option<ElementList>) -> Vec<String> {
	match elements {
		Some(list) => list.filter_map(|element| element.text()).collect(),
		None => Vec::new(),
	}
}

fn parse_chapter_number(title: &str) -> Option<f32> {
	let rest = match title.find('第') {
		Some(index) => &title[index + '第'.len_utf8()..],
		None => {
			let start = title.find(|c: char| c.is_ascii_digit())?;
			&title[start..]
		}
	};
	let end = rest
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(rest.len());
	rest[..end].trim_end_matches('.').parse().ok()
}

/// `.cover_box > a` grid used by search and the latest-updates listing.
pub fn parse_mangas_page(doc: &Document) -> MangaPageResult {
	let entries = doc
		.select(".cover_box > a")
		.map(|list| {
			list.filter_map(|a| {
				let img = a.select_first(".cover")?;
				let href = a.attr("abs:href")?;
				Some(Manga {
					key: key_from_url(&href),
					title: a.attr("title").unwrap_or_default(),
					cover: img.attr("abs:data-src"),
					url: Some(href),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default();

	// The second-to-last pagination link is the "next" one; it loses the
	// `active` class once there are further pages.
	let has_next_page = doc
		.select_first(".pagination_box > .content_box > div:nth-last-child(2) > a")
		.is_some_and(|element| !element.has_class("active"));

	MangaPageResult {
		entries,
		has_next_page,
	}
}

/// `.rank_item > a` grid used by the popular listing.
pub fn parse_rank(doc: &Document) -> Vec<Manga> {
	doc.select(".rank_item > a")
		.map(|list| {
			list.filter_map(|a| {
				let img = a.select_first(".cover > img")?;
				let href = a.attr("abs:href")?;
				let mut authors = joined_text(a.select(".author a"));
				if authors.is_empty() {
					authors = a
						.select_first(".author")
						.and_then(|element| element.text())
						.unwrap_or_default();
				}
				Some(Manga {
					key: key_from_url(&href),
					title: img.attr("alt").unwrap_or_default(),
					cover: img.attr("abs:data-src"),
					authors: (!authors.is_empty()).then_some(vec![authors]),
					description: a.select_first(".brief").and_then(|element| element.text()),
					url: Some(href),
					..Default::default()
				})
			})
			.collect()
		})
		.unwrap_or_default()
}

/// Details and the (oldest-first) chapter list from the manga page. The key
/// and url are filled in by the caller because `doc.location()` is not exposed.
pub fn parse_manga_detail(doc: &Document) -> (Manga, Vec<Chapter>) {
	let cover = doc
		.select_first(".comic_cover_box > .flex_box > img")
		.and_then(|img| img.attr("abs:data-src"));
	let title = doc
		.select_first(".comic_title")
		.and_then(|element| element.text())
		.unwrap_or_default();
	let note = doc
		.select_first(".translation_agency_box")
		.and_then(|element| element.text());
	let base_intro = doc
		.select_first(".intro_box > .txt")
		.and_then(|element| element.text())
		.map(|text| match text.split_once("作品介绍：") {
			Some((_, rest)) => rest.to_string(),
			None => text,
		})
		.unwrap_or_default();
	let description = match note {
		Some(note) => format!("{base_intro}\n\n*译者/机构：{note}*"),
		None => base_intro,
	};

	let author_el = doc.select_first(".author");
	let from_links = {
		let joined = joined_text(author_el.as_ref().and_then(|element| element.select("a")));
		(!joined.is_empty()).then_some(joined)
	};
	let authors = from_links.or_else(|| {
		author_el
			.and_then(|element| element.text())
			.filter(|text| !text.is_empty())
	});

	let status = match doc
		.select_first(".state_box > span:nth-of-type(2)")
		.and_then(|element| element.text())
		.as_deref()
	{
		Some("连载中") => MangaStatus::Ongoing,
		Some("完结") => MangaStatus::Completed,
		_ => MangaStatus::Unknown,
	};
	let tags = text_list(doc.select(".tag_box a"));

	let manga = Manga {
		title,
		cover,
		authors: authors.map(|author| vec![author]),
		description: Some(description),
		tags: (!tags.is_empty()).then_some(tags),
		status,
		..Default::default()
	};

	// Paid chapters are prefixed with the upstream coin marker (U+1FA99).
	const PAID_MARK: &str = "\u{1FA99} ";
	let mut chapters: Vec<Chapter> = doc
		.select(".catalog_box a")
		.map(|list| {
			list.enumerate()
				.filter_map(|(index, a)| {
					let href = a.attr("abs:href")?;
					let price = a
						.select_first("span:last-child")
						.and_then(|element| element.text())
						.and_then(|text| text.parse::<f32>().ok());
					let title_text = a
						.select_first(".title")
						.and_then(|element| element.text())
						.unwrap_or_default();
					let title = match price {
						Some(_) => format!("{PAID_MARK}{title_text}"),
						None => title_text.clone(),
					};
					Some(Chapter {
						key: key_from_url(&href),
						title: Some(title),
						chapter_number: parse_chapter_number(&title_text)
							.or(Some((index + 1) as f32)),
						scanlators: price.map(|value| vec![format!("￥{value}")]),
						url: Some(href),
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default();
	chapters.reverse();

	(manga, chapters)
}

pub fn chapter_code(doc: &Document) -> Option<String> {
	doc.select_first(".comic_chapter_box")
		.and_then(|element| element.attr("code"))
}

/// The `#content > img` reader images, with the `encrypted-image` class flag.
pub fn page_images(doc: &Document) -> Vec<(String, bool)> {
	doc.select("#content > img")
		.map(|list| {
			list.filter_map(|img| {
				Some((img.attr("abs:data-src")?, img.has_class("encrypted-image")))
			})
			.collect()
		})
		.unwrap_or_default()
}
