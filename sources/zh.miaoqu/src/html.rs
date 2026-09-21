use crate::net::MOBILE_URL;
use aidoku::{
	Chapter, Manga, MangaPageResult, Page, PageContent, Result, SelectFilter,
	alloc::{String, Vec, format, string::ToString as _, vec},
	error,
	imports::html::{Document, Element},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

// Chapter images are encrypted with one of these keys, picked by the last digit of
// the chapter id, and the key is then xor-ed over the second base64 layer.
const KEYS: [&str; 10] = [
	"8-bXd9iN",
	"8-RXyjry",
	"8-oYvwVy",
	"8-4ZY57U",
	"8-mbJpU7",
	"8-6MM2Ei",
	"8-54TiQr",
	"8-Ph5xx9",
	"8-bYgePR",
	"8-Z9A3bW",
];

#[derive(Deserialize)]
struct Image {
	url: String,
}

pub trait MangaList {
	fn manga_page_result(&self, page: i32) -> Result<MangaPageResult>;
}

impl MangaList for Document {
	fn manga_page_result(&self, page: i32) -> Result<MangaPageResult> {
		let entries = self
			.select("#mangawrap > li")
			.map(|elements| elements.filter_map(manga_from_element).collect())
			.unwrap_or_default();
		// The next button keeps pointing at the current page once there is none left.
		let current = page.to_string();
		let has_next_page = self
			.select_first("#next")
			.and_then(|button| button.attr("href"))
			.is_some_and(|href| {
				href.rsplit_once('/')
					.is_some_and(|(_, next)| next != current.as_str())
			});

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

fn manga_from_element(element: Element) -> Option<Manga> {
	let link = element.select_first("a.manga-img")?;

	Some(Manga {
		key: link.attr("href")?.trim_start_matches('/').to_string(),
		title: element.select_first(".manga-name")?.text()?,
		cover: link
			.attr("style")
			.and_then(|style| style.split_once("background: url("))
			.and_then(|(_, url)| url.split_once(')'))
			.map(|(url, _)| url.to_string()),
		authors: element
			.select_first(".manga-author")
			.and_then(|author| author.text())
			.map(|author| vec![author]),
		..Default::default()
	})
}

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga);
	fn chapters(&self) -> Vec<Chapter>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) {
		let Some(infobox) = self.select_first(".infobox") else {
			return;
		};

		let title = infobox
			.select_first(".title")
			.and_then(|title| title.text());
		if let Some(title) = title {
			manga.title = title;
		}
		manga.cover = infobox
			.select_first("img")
			.and_then(|cover| cover.attr("abs:src"));
		manga.description = self
			.select_first(".text")
			.and_then(|description| description.text());

		if let Some(tages) = infobox.select(".tage") {
			for element in tages {
				let Some(text) = element.text() else {
					continue;
				};

				if let Some(author) = text.strip_prefix("作者：") {
					manga.authors = Some(vec![author.trim_start().to_string()]);
				} else if text.starts_with("类型：") {
					manga.tags = element
						.select("a")
						.map(|tags| tags.filter_map(|tag| tag.text()).collect());
				} else if text.starts_with("更新于") {
					let description = match manga.description.take() {
						Some(description) => format!("{text}\n\n{description}"),
						None => text,
					};
					manga.description = Some(description);
				}
			}
		}

		let url = format!("{MOBILE_URL}/{}", manga.key);
		manga.url = Some(url);
	}

	fn chapters(&self) -> Vec<Chapter> {
		// The listed order is the reading order, and titles mix chapters with
		// volumes (`第17卷`), so no chapter number is derived from them.
		self.select("ul.list > li")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let link = element.select_first("a")?;
						let key = link.attr("href")?;
						Some(Chapter {
							key: key.trim_start_matches('/').to_string(),
							title: link.text(),
							..Default::default()
						})
					})
					.collect()
			})
			.unwrap_or_default()
	}
}

pub trait ChapterPage {
	fn pages(&self, key: &str) -> Result<Vec<Page>>;
	fn manga_key(&self) -> Result<String>;
}

impl ChapterPage for Document {
	fn pages(&self, key: &str) -> Result<Vec<Page>> {
		let cid: usize = key
			.rsplit_once('/')
			.map_or(key, |(_, cid)| cid)
			.trim_end_matches(".html")
			.parse()
			.map_err(|_| error!("Invalid chapter id: `{key}`"))?;
		let xor_key = KEYS[cid % 10].as_bytes();

		let data = self
			.select("script")
			.and_then(|scripts| {
				scripts
					.filter_map(|script| script.data())
					.find(|script| script.contains("var DATA="))
			})
			.and_then(|script| {
				let (_, data) = script.split_once("var DATA='")?;
				let (data, _) = data.split_once('\'')?;
				Some(data.to_string())
			})
			.ok_or_else(|| error!("No image data found on chapter page"))?;

		let encrypted = STANDARD
			.decode(data)
			.map_err(|_| error!("Invalid image data"))?;
		let decrypted: Vec<u8> = encrypted
			.iter()
			.enumerate()
			.map(|(index, byte)| *byte ^ xor_key[index & 7])
			.collect();
		let inner = STANDARD
			.decode(decrypted)
			.map_err(|_| error!("Invalid image data"))?;
		let text = String::from_utf8(inner)
			.map_err(|_| error!("Invalid image data"))?;
		let images: Vec<Image> = serde_json::from_str(&text)?;

		Ok(images
			.into_iter()
			.map(|image| Page {
				content: PageContent::url(image.url),
				..Default::default()
			})
			.collect())
	}

	fn manga_key(&self) -> Result<String> {
		self.select("div.manga-btns-2 a")
			.and_then(|elements| {
				elements
					.filter_map(|element| element.attr("href"))
					.find(|href| href.as_str() != "/")
			})
			.map(|href| href.trim_start_matches('/').to_string())
			.ok_or_else(|| error!("No manga link found on chapter page"))
	}
}

pub trait GenresPage {
	fn genre_filter(&self) -> Result<SelectFilter>;
}

impl GenresPage for Document {
	fn genre_filter(&self) -> Result<SelectFilter> {
		let (mut options, mut ids) = self
			.select(".ticai a")
			.ok_or_else(|| error!("No genre list found on category page"))?
			.filter_map(|element| {
				let href = element.attr("href")?;
				let id = href.rsplit_once('/')?.1;
				if !href.contains("/tags/") {
					return None;
				}
				Some((element.text()?, format!("tags/{id}")))
			})
			.collect::<(Vec<_>, Vec<_>)>();

		options.insert(0, "全部".into());
		ids.insert(0, "".into());

		Ok(SelectFilter {
			id: "题材".into(),
			title: Some("题材".into()),
			is_genre: true,
			options,
			ids: Some(ids),
			..Default::default()
		})
	}
}
