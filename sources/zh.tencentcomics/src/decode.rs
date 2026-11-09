use aidoku::{
	Page, PageContent, Result,
	alloc::{String, Vec, format},
	error,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ChapterData {
	pub chapter: ChapterInfo,
	#[serde(default)]
	pub picture: Vec<PageInfo>,
}

#[derive(Deserialize)]
pub struct ChapterInfo {
	#[serde(rename = "canRead", default)]
	pub can_read: bool,
}

#[derive(Deserialize)]
pub struct PageInfo {
	pub url: String,
}

/// The reader page embeds a base64 JSON blob with noise characters inserted.
/// A JS "nonce" expression describes the positions of those noise characters.
/// The upstream extension evaluates it with QuickJS; here the (tiny, fixed)
/// expression grammar is evaluated directly so no JS engine is needed.
pub fn decode_pages(html: &str) -> Result<Vec<Page>> {
	let raw = extract_data(html).ok_or_else(|| error!("Chapter DATA not found"))?;
	let expr = extract_nonce_expr(html).ok_or_else(|| error!("Chapter nonce not found"))?;
	let nonce = eval_nonce_expr(expr).ok_or_else(|| error!("Invalid chapter nonce"))?;

	let mut chars: Vec<u8> = raw.as_bytes().to_vec();
	for (offset, len) in nonce_groups(&nonce).iter().rev() {
		let start = (*offset & 255).min(chars.len());
		let end = (start + *len).min(chars.len());
		if start < end {
			chars.drain(start..end);
		}
	}

	let json = base64_decode(&chars).ok_or_else(|| error!("Invalid chapter data"))?;
	let data: ChapterData =
		serde_json::from_slice(&json).map_err(|_| error!("Invalid chapter JSON"))?;

	if !data.chapter.can_read {
		return Err(error!("此章节为付费内容"));
	}

	Ok(data
		.picture
		.into_iter()
		.map(|page| Page {
			content: PageContent::url(page.url),
			..Default::default()
		})
		.collect())
}

fn extract_data(html: &str) -> Option<String> {
	let after = html.rsplit_once("var DATA =")?.1;
	let before = after.split_once("PRELOAD_NUM")?.0.trim();
	let mut s = before;
	if s.starts_with('\'') {
		s = &s[1..];
	}
	if s.ends_with('\'') {
		s = &s[..s.len() - 1];
	}
	if s.ends_with(',') {
		s = &s[..s.len() - 1];
	}
	Some(String::from(s.trim()))
}

fn extract_nonce_expr(html: &str) -> Option<&str> {
	let rhs = html.rsplit_once("window[")?.1.split_once("] = ")?.1;
	let expr = rhs.split_once("</script>")?.0.trim();
	Some(expr.trim_end_matches(';').trim())
}

/// Evaluate the concatenation expression that produces the nonce.
/// String literals are appended verbatim; `eval("...")` arguments are
/// evaluated as arithmetic/bool probes (the browser stubs make every
/// `document`/`window` reference truthy) and their numeric values appended.
fn eval_nonce_expr(expr: &str) -> Option<String> {
	let bytes = expr.as_bytes();
	let mut out = String::new();
	let mut i = 0;
	while i < bytes.len() {
		if bytes[i] == b'"' {
			let start = i + 1;
			let mut j = start;
			while j < bytes.len() {
				if bytes[j] == b'\\' {
					j += 2;
					continue;
				}
				if bytes[j] == b'"' {
					break;
				}
				j += 1;
			}
			let s = expr.get(start..j.min(bytes.len()))?;
			let is_eval =
				start >= 5 && expr.as_bytes().get(start - 5..start) == Some(&b"eval("[..]);
			if is_eval {
				let value = eval_arith(s).unwrap_or(0.0);
				out.push_str(&format_number(value));
			} else {
				out.push_str(s);
			}
			i = j.saturating_add(1);
		} else {
			i += 1;
		}
	}
	Some(out)
}

fn format_number(value: f64) -> String {
	if value.is_finite() && value == (value as i64) as f64 {
		format!("{}", value as i64)
	} else {
		format!("{value}")
	}
}

/// Collect `\\d+[a-zA-Z]+` groups: (offset, number of noise characters).
fn nonce_groups(nonce: &str) -> Vec<(usize, usize)> {
	let bytes = nonce.as_bytes();
	let mut groups = Vec::new();
	let mut i = 0;
	while i < bytes.len() {
		if bytes[i].is_ascii_digit() {
			let digit_start = i;
			while i < bytes.len() && bytes[i].is_ascii_digit() {
				i += 1;
			}
			if i < bytes.len() && bytes[i].is_ascii_alphabetic() {
				let number: usize = core::str::from_utf8(&bytes[digit_start..i])
					.ok()
					.and_then(|s| s.parse().ok())
					.unwrap_or(0);
				let letter_start = i;
				while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
					i += 1;
				}
				groups.push((number & 255, i - letter_start));
			}
		} else {
			i += 1;
		}
	}
	groups
}

struct Parser<'a> {
	bytes: &'a [u8],
	pos: usize,
}

impl Parser<'_> {
	fn skip_ws(&mut self) {
		while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
			self.pos += 1;
		}
	}

	fn expr(&mut self) -> Option<f64> {
		let mut value = self.term()?;
		loop {
			self.skip_ws();
			match self.bytes.get(self.pos).copied() {
				Some(b'+') => {
					self.pos += 1;
					value += self.term()?;
				}
				Some(b'-') => {
					self.pos += 1;
					value -= self.term()?;
				}
				_ => break,
			}
		}
		Some(value)
	}

	fn term(&mut self) -> Option<f64> {
		let mut value = self.unary()?;
		loop {
			self.skip_ws();
			match self.bytes.get(self.pos).copied() {
				Some(b'*') => {
					self.pos += 1;
					value *= self.unary()?;
				}
				Some(b'/') => {
					self.pos += 1;
					value /= self.unary()?;
				}
				_ => break,
			}
		}
		Some(value)
	}

	fn unary(&mut self) -> Option<f64> {
		self.skip_ws();
		match self.bytes.get(self.pos).copied() {
			Some(b'!') => {
				self.pos += 1;
				let value = self.unary()?;
				Some(if value != 0.0 { 0.0 } else { 1.0 })
			}
			Some(b'+') => {
				self.pos += 1;
				self.unary()
			}
			Some(b'-') => {
				self.pos += 1;
				Some(-self.unary()?)
			}
			_ => self.primary(),
		}
	}

	fn primary(&mut self) -> Option<f64> {
		self.skip_ws();
		match self.bytes.get(self.pos).copied() {
			Some(b'(') => {
				self.pos += 1;
				let value = self.expr()?;
				self.skip_ws();
				if self.bytes.get(self.pos) == Some(&b')') {
					self.pos += 1;
				}
				Some(value)
			}
			Some(c) if c.is_ascii_digit() => {
				let start = self.pos;
				while self.pos < self.bytes.len()
					&& (self.bytes[self.pos].is_ascii_digit() || self.bytes[self.pos] == b'.')
				{
					self.pos += 1;
				}
				core::str::from_utf8(&self.bytes[start..self.pos])
					.ok()?
					.parse()
					.ok()
			}
			Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {
				// Under the upstream browser stubs every document/window
				// reference resolves to a truthy object.
				while self.pos < self.bytes.len()
					&& (self.bytes[self.pos].is_ascii_alphanumeric()
						|| matches!(self.bytes[self.pos], b'_' | b'$' | b'.'))
				{
					self.pos += 1;
				}
				Some(1.0)
			}
			_ => None,
		}
	}
}

fn eval_arith(expr: &str) -> Option<f64> {
	let mut parser = Parser {
		bytes: expr.as_bytes(),
		pos: 0,
	};
	parser.expr()
}

fn base64_value(c: u8) -> Option<u8> {
	match c {
		b'A'..=b'Z' => Some(c - b'A'),
		b'a'..=b'z' => Some(c - b'a' + 26),
		b'0'..=b'9' => Some(c - b'0' + 52),
		b'+' => Some(62),
		b'/' => Some(63),
		_ => None,
	}
}

fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
	let clean: Vec<u8> = input
		.iter()
		.copied()
		.filter(|c| !c.is_ascii_whitespace() && *c != b'=')
		.collect();
	let mut out = Vec::new();
	let mut buf = [0u8; 4];
	let mut n = 0;
	for &c in &clean {
		buf[n] = base64_value(c)?;
		n += 1;
		if n == 4 {
			out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
			out.push(((buf[1] as u32) << 4 | (buf[2] as u32) >> 2) as u8);
			out.push(((buf[2] as u32) << 6 | buf[3] as u32) as u8);
			n = 0;
		}
	}
	if n >= 2 {
		out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
	}
	if n >= 3 {
		out.push(((buf[1] as u32) << 4 | (buf[2] as u32) >> 2) as u8);
	}
	Some(out)
}
