use crate::net::BASE_URL;
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result,
	alloc::{String, Vec, format, string::ToString as _, vec},
	error,
	imports::html::{Document, Element},
};

pub trait MangaList {
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaList for Document {
	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let entries = self
			.select("article.card")
			.map(|elements| elements.filter_map(manga_from_card).collect())
			.unwrap_or_default();
		let has_next_page = self.select("nav.pager a").is_some_and(|elements| {
			elements
				.filter_map(|element| element.text())
				.any(|text| text == ">")
		});

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

fn manga_from_card(card: Element) -> Option<Manga> {
	let key = card.select_first("a.cover-wrap")?.attr("href")?;

	Some(Manga {
		key: key.trim_start_matches('/').to_string(),
		title: card.select_first("h3 a")?.text()?,
		cover: card
			.select_first("img.cover")
			.and_then(|cover| cover.attr("abs:src")),
		authors: card
			.select_first("div.meta")
			.and_then(|meta| meta.text())
			.and_then(|meta| {
				let author = meta.split('·').next()?.trim();
				if author.is_empty() {
					None
				} else {
					Some(vec![author.to_string()])
				}
			}),
		..Default::default()
	})
}

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga);
	fn chapters(&self) -> Vec<Chapter>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) {
		if let Some(title) = self
			.select_first("div.mobile-comic-title")
			.and_then(|title| title.text())
		{
			manga.title = title;
		}

		manga.cover = self
			.select_first("img.mobile-comic-cover")
			.and_then(|cover| cover.attr("abs:src"));
		manga.description = self
			.select_first("p.mobile-comic-desc")
			.and_then(|description| description.text());
		manga.tags = self
			.select_first("p.mobile-comic-tags")
			.and_then(|tags| tags.text())
			.map(|tags| {
				tags.split('/')
					.map(|tag| tag.trim().to_string())
					.filter(|tag| !tag.is_empty())
					.collect()
			});
		manga.authors = self.select("div.cinema-strip > div").and_then(|elements| {
			elements
				.filter(|element| {
					element
						.select_first("span")
						.and_then(|label| label.text())
						.as_deref()
						== Some("作者")
				})
				.find_map(|element| element.select_first("b").and_then(|author| author.text()))
				.filter(|author| !author.is_empty())
				.map(|author| vec![author])
		});

		let meta = self
			.select_first("p.mobile-comic-meta")
			.and_then(|meta| meta.text())
			.unwrap_or_default();
		manga.status = if meta.contains("完结") {
			MangaStatus::Completed
		} else if meta.contains("连载") {
			MangaStatus::Ongoing
		} else {
			MangaStatus::Unknown
		};

		let url = format!("{BASE_URL}/{}", manga.key);
		manga.url = Some(url);
	}

	fn chapters(&self) -> Vec<Chapter> {
		let mut chapters: Vec<Chapter> = self
			.select("section.mobile-comic-all-chapters div.mobile-chapter-grid a")
			.map(|elements| {
				elements
					.filter_map(|element| {
						let key = element.attr("href")?;
						let title = element.text();
						Some(Chapter {
							key: key.trim_start_matches('/').to_string(),
							chapter_number: title.as_deref().and_then(leading_number),
							title,
							..Default::default()
						})
					})
					.collect()
			})
			.unwrap_or_default();

		// The grid lists the newest chapter first.
		chapters.reverse();
		for (index, chapter) in chapters.iter_mut().enumerate() {
			chapter.chapter_number.get_or_insert((index + 1) as f32);
		}

		chapters
	}
}

// Chapter titles are numbered like `334 鬼梦貘`, so the leading number can be
// read directly, falling back to the position for the rest.
fn leading_number(title: &str) -> Option<f32> {
	title.split_whitespace().next()?.parse().ok()
}

pub trait ChapterPage {
	fn pages(&self) -> Vec<Page>;
	fn manga_key(&self) -> Result<String>;
}

impl ChapterPage for Document {
	fn pages(&self) -> Vec<Page> {
		self.select("section.reader-images img")
			.map(|elements| {
				elements
					.filter_map(|element| element.attr("abs:src"))
					.map(|url| Page {
						content: PageContent::url(url),
						..Default::default()
					})
					.collect()
			})
			.unwrap_or_default()
	}

	fn manga_key(&self) -> Result<String> {
		self.select("section.reader-finish a")
			.and_then(|elements| {
				elements
					.filter_map(|element| element.attr("href"))
					.find(|href| href.contains("comic.php"))
			})
			.map(|href| href.trim_start_matches('/').to_string())
			.ok_or_else(|| error!("No manga link found on chapter page"))
	}
}
