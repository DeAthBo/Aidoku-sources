use aidoku::alloc::String;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchDto {
	pub slug: String,
	pub title: String,
	pub cover: String,
}
