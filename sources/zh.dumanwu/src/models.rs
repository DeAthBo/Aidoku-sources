use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ResponseDto {
	pub data: Vec<ChapterDto>,
}

#[derive(Deserialize)]
pub struct ChapterDto {
	pub chapterid: String,
	pub chaptername: String,
}
