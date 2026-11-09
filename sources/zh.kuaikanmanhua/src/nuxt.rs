use aidoku::{
	Result,
	alloc::{String, Vec, collections::BTreeMap},
	error,
};

/// Decode a Nuxt 2 compressed `window.__NUXT__=(function(a,b,...){return OBJ}(ARGS));`
/// payload into a plain JSON string.
///
/// The upstream extension runs this through QuickJS. Nuxt's `devalue` output is
/// mechanical: every constant is passed as an IIFE argument and referenced by a
/// short parameter name inside the returned object. We substitute those names
/// back with their literal values and normalize the JS object into JSON, so no
/// JavaScript engine is required in the WASM source.
pub fn decode(script: &str) -> Result<String> {
	let bytes = script.as_bytes();

	let fn_pos = find_sub(bytes, b"function(").ok_or_else(|| error!("NUXT: function not found"))?;
	let params_start = fn_pos + b"function(".len();
	let params_end = params_start
		+ bytes[params_start..]
			.iter()
			.position(|&b| b == b')')
			.ok_or_else(|| error!("NUXT: params not terminated"))?;
	let params: Vec<&[u8]> = bytes[params_start..params_end]
		.split(|&b| b == b',')
		.map(trim_bytes)
		.collect();

	let mut body_start = params_end + 1;
	while body_start < bytes.len() && bytes[body_start].is_ascii_whitespace() {
		body_start += 1;
	}
	if bytes.get(body_start) != Some(&b'{') {
		return Err(error!("NUXT: body not found"));
	}
	let body_end = find_matching(bytes, body_start, b'{', b'}')
		.ok_or_else(|| error!("NUXT: body not terminated"))?;
	let body = &bytes[body_start + 1..body_end];
	let return_pos = rfind_sub(body, b"return ").ok_or_else(|| error!("NUXT: return not found"))?;
	let obj = trim_bytes(&body[return_pos + b"return ".len()..]);

	let mut arg_start = body_end + 1;
	while arg_start < bytes.len() && bytes[arg_start].is_ascii_whitespace() {
		arg_start += 1;
	}
	if bytes.get(arg_start) != Some(&b'(') {
		return Err(error!("NUXT: args not found"));
	}
	let arg_end = find_matching(bytes, arg_start, b'(', b')')
		.ok_or_else(|| error!("NUXT: args not terminated"))?;
	let args = split_top(&bytes[arg_start + 1..arg_end]);

	let mut values: BTreeMap<&[u8], String> = BTreeMap::new();
	for (index, param) in params.iter().enumerate() {
		let value = args
			.get(index)
			.map_or_else(|| String::from("null"), |arg| arg_to_json(arg));
		values.insert(param, value);
	}

	Ok(obj_to_json(obj, &values))
}

fn find_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
	if needle.is_empty() {
		return Some(0);
	}
	haystack.windows(needle.len()).position(|w| w == needle)
}

fn rfind_sub(haystack: &[u8], needle: &[u8]) -> Option<usize> {
	if needle.is_empty() {
		return Some(haystack.len());
	}
	haystack.windows(needle.len()).rposition(|w| w == needle)
}

fn trim_bytes(s: &[u8]) -> &[u8] {
	let mut start = 0;
	let mut end = s.len();
	while start < end && s[start].is_ascii_whitespace() {
		start += 1;
	}
	while end > start && s[end - 1].is_ascii_whitespace() {
		end -= 1;
	}
	&s[start..end]
}

/// Find the matching closing delimiter, skipping over quoted strings.
fn find_matching(bytes: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
	let mut depth = 0i32;
	let mut i = start;
	let mut quote: Option<u8> = None;
	while i < bytes.len() {
		let c = bytes[i];
		if let Some(q) = quote {
			if c == b'\\' {
				i += 2;
				continue;
			}
			if c == q {
				quote = None;
			}
		} else {
			match c {
				b'"' | b'\'' => quote = Some(c),
				_ if c == open => depth += 1,
				_ if c == close => {
					depth -= 1;
					if depth == 0 {
						return Some(i);
					}
				}
				_ => {}
			}
		}
		i += 1;
	}
	None
}

/// Split on top-level commas (outside strings and nested brackets).
fn split_top(s: &[u8]) -> Vec<&[u8]> {
	let mut parts = Vec::new();
	let mut start = 0;
	let mut depth = 0i32;
	let mut i = 0;
	let mut quote: Option<u8> = None;
	while i < s.len() {
		let c = s[i];
		if let Some(q) = quote {
			if c == b'\\' {
				i += 2;
				continue;
			}
			if c == q {
				quote = None;
			}
		} else {
			match c {
				b'"' | b'\'' => quote = Some(c),
				b'(' | b'[' | b'{' => depth += 1,
				b')' | b']' | b'}' => depth -= 1,
				b',' if depth == 0 => {
					parts.push(trim_bytes(&s[start..i]));
					start = i + 1;
				}
				_ => {}
			}
		}
		i += 1;
	}
	parts.push(trim_bytes(&s[start..]));
	parts
}

fn is_number(t: &[u8]) -> bool {
	if t.is_empty() {
		return false;
	}
	let mut i = usize::from(t[0] == b'-');
	let mut dot = false;
	let mut digit = false;
	while i < t.len() {
		match t[i] {
			b'0'..=b'9' => digit = true,
			b'.' if !dot => dot = true,
			_ => return false,
		}
		i += 1;
	}
	digit
}

fn arg_to_json(arg: &[u8]) -> String {
	let t = trim_bytes(arg);
	if t.is_empty() {
		return String::from("null");
	}
	if t.len() >= 2 {
		let quoted =
			(t[0] == b'"' && t[t.len() - 1] == b'"') || (t[0] == b'\'' && t[t.len() - 1] == b'\'');
		if quoted {
			if t[0] == b'\'' {
				let inner = String::from_utf8_lossy(&t[1..t.len() - 1]);
				return serde_json::to_string(inner.as_ref())
					.unwrap_or_else(|_| String::from("null"));
			}
			return String::from_utf8(t.to_vec()).unwrap_or_else(|_| String::from("null"));
		}
	}
	if t == b"void 0".as_slice()
		|| t == b"undefined".as_slice()
		|| t == b"NaN".as_slice()
		|| t == b"null".as_slice()
	{
		return String::from("null");
	}
	if t.starts_with(b"Array(") {
		return String::from("[]");
	}
	if is_number(t) || t == b"true".as_slice() || t == b"false".as_slice() {
		return String::from_utf8_lossy(t).into_owned();
	}
	String::from("null")
}

fn obj_to_json(obj: &[u8], values: &BTreeMap<&[u8], String>) -> String {
	let mut out: Vec<u8> = Vec::new();
	let mut i = 0;
	let mut quote: Option<u8> = None;
	while i < obj.len() {
		let c = obj[i];
		if let Some(q) = quote {
			if c == b'\\' && i + 1 < obj.len() {
				out.push(b'\\');
				out.push(obj[i + 1]);
				i += 2;
				continue;
			}
			if c == q {
				out.push(b'"');
				quote = None;
			} else if c == b'"' && q == b'\'' {
				out.extend_from_slice(b"\\\"");
			} else {
				out.push(c);
			}
			i += 1;
			continue;
		}
		if c == b'"' || c == b'\'' {
			quote = Some(c);
			out.push(b'"');
			i += 1;
			continue;
		}
		if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
			let start = i;
			while i < obj.len() && (obj[i].is_ascii_alphanumeric() || matches!(obj[i], b'_' | b'$'))
			{
				i += 1;
			}
			let word = &obj[start..i];
			let mut k = i;
			while k < obj.len() && obj[k].is_ascii_whitespace() {
				k += 1;
			}
			if k < obj.len() && obj[k] == b':' {
				out.push(b'"');
				out.extend_from_slice(word);
				out.push(b'"');
				continue;
			}
			match word {
				b"void" => {
					while i < obj.len() && obj[i].is_ascii_whitespace() {
						i += 1;
					}
					if i < obj.len() && obj[i] == b'0' {
						i += 1;
					}
					out.extend_from_slice(b"null");
				}
				b"Array" => {
					while i < obj.len() && obj[i] != b'(' {
						i += 1;
					}
					if i < obj.len()
						&& let Some(end) = find_matching(obj, i, b'(', b')')
					{
						i = end + 1;
					}
					out.extend_from_slice(b"[]");
				}
				b"true" | b"false" | b"null" => out.extend_from_slice(word),
				_ => out.extend_from_slice(
					values
						.get(word)
						.map_or(b"null".as_slice(), String::as_bytes),
				),
			}
			continue;
		}
		out.push(c);
		i += 1;
	}
	String::from_utf8(out).unwrap_or_default()
}
