//! `JSON_SearchConst` and `JSON_Iterate`: the query half of coreJSON.
//!
//! The validator answers one question about a whole document. This answers
//! questions about its *parts*, and it does so **without building anything** —
//! no tree, no allocation, no copy. A query returns a sub-slice of the buffer
//! the caller already has, which is why coreJSON exists at all on a part with
//! 64 KiB of RAM.
//!
//! # Indices, not pointers
//!
//! The C hands back a `const char *` into the caller's buffer and a length.
//! Here the same answer is a `&[u8]` borrowed from the input, so the lifetime
//! that the C leaves to the reader's care is checked by the compiler instead.
//! Internally everything is an index, per the family rule: handles are
//! indices, never pointers.
//!
//! # Two refusal types rather than one
//!
//! The C has a single `JSONStatus_t` and each function documents the subset it
//! can return. [`search`] can only ever answer `NotFound` or `BadParameter`,
//! and [`iterate`] adds `IllegalDocument`, so they get different types and the
//! impossible variant is not there to be matched on. That is the same move the
//! validator makes with [`Validity`](crate::Validity), and the same one
//! `heap_1` makes by refusing to have a `free`.
//!
//! # This does NOT validate first
//!
//! Neither does the C. A query walks whatever bytes it is given, so a
//! malformed document produces a refusal or a partial answer rather than a
//! diagnosis. Call [`validate`](crate::validate) if you need one. The
//! differential leans on this deliberately: it runs both entry points over all
//! 188 deliberately-malformed corpus files, because walking a broken document
//! is exactly where a reimplementation runs off the end.

use crate::validate::{
    Validity, at, skip_any_scalar, skip_collection, skip_digits_value, skip_space,
    skip_space_and_comma, skip_string,
};

/// `JSON_QUERY_KEY_SEPARATOR`, the character between the parts of a query.
pub const QUERY_KEY_SEPARATOR: u8 = b'.';

/// `JSONTypes_t`, for the types a found value can actually have.
///
/// The C's enum also carries `JSONInvalid`, which is what its callers use to
/// initialise the out-parameter before the call. Nothing here ever returns it,
/// so it is not a variant: a [`Match`] always has a real type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A quoted string. The quotes are stripped from [`Match::value`].
    String,
    /// Any number, including negative and exponent forms.
    Number,
    /// `true`.
    True,
    /// `false`.
    False,
    /// `null`.
    Null,
    /// An object, reported whole, braces included.
    Object,
    /// An array, reported whole, brackets included.
    Array,
}

/// `getType`: the type of a value, from its first byte alone.
///
/// Anything that is not one of the six punctuation or literal starts is a
/// number — the C does not check, because by the time a value has been
/// scanned it is already known to be one of the seven.
const fn kind_of(c: u8) -> Kind {
    match c {
        b'"' => Kind::String,
        b'{' => Kind::Object,
        b'[' => Kind::Array,
        b't' => Kind::True,
        b'f' => Kind::False,
        b'n' => Kind::Null,
        _ => Kind::Number,
    }
}

/// Why a [`search`] found nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotFound {
    /// `JSONNotFound`: the document is walkable and the query is not in it.
    Missing,
    /// `JSONBadParameter`: an empty buffer, an empty query, a query with an
    /// empty part or a trailing separator, or an array index that is
    /// malformed or too large to be one.
    BadQuery,
}

/// Why an [`iterate`] stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// `JSONNotFound`: the collection has no further values.
    Exhausted,
    /// `JSONIllegalDocument`: the buffer does not begin with `[` or `{`.
    NotACollection,
    /// `JSONBadParameter`: an empty buffer, or a cursor outside it.
    BadCursor,
}

/// A value found by [`search`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match<'a> {
    /// Where the value starts in the buffer that was searched.
    ///
    /// For a [`Kind::String`] this is the first byte **inside** the quotes,
    /// matching the C, which advances the pointer and shortens the length by
    /// two so that a caller gets the text rather than the literal.
    pub offset: usize,
    /// The value itself, borrowed from the buffer that was searched.
    pub value: &'a [u8],
    /// What kind of value it is.
    pub kind: Kind,
}

/// One key-value pair, or one array element, from [`iterate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair<'a> {
    /// The key, with its quotes stripped — or `None` for an array element.
    ///
    /// The C signals "no key" with a NULL pointer, and derives it from the key
    /// index being zero. That conflation is harmless there because an object's
    /// first key can never start at offset 0 (a `{` precedes it), and it is
    /// reproduced here exactly rather than improved on.
    pub key: Option<&'a [u8]>,
    /// Where the key starts, or 0 when there is none.
    ///
    /// The C hands back a pointer and leaves the caller to subtract; carrying
    /// the index means nobody has to do pointer arithmetic to find out where
    /// a key was. The first version of the differential did exactly that, and
    /// needing it was the sign this field was missing.
    pub key_offset: usize,
    /// Where the value starts, with the same string adjustment as [`Match`].
    pub offset: usize,
    /// The value itself.
    pub value: &'a [u8],
    /// What kind of value it is.
    pub kind: Kind,
}

// ---- the walkers ---------------------------------------------------------

/// `nextValue`: the next value, scalar or collection, and where it ends.
fn next_value(buf: &[u8], start: &mut usize, max: usize) -> Option<(usize, usize)> {
    let mut i = *start;
    let value_start = i;

    // A scalar first, then a collection, which is the C's order.
    //
    // It was worth claiming that the order MATTERS, and it does not: swapping
    // these two leaves all 3,089 lines of the query differential identical,
    // across 30 documents and all 318 corpus files. The two scanners are
    // disjoint on their first byte and neither moves the cursor when it
    // fails, which is what makes the order free -- and
    // `the_two_scanners_are_disjoint` below pins both halves of that, so it
    // stays a property rather than a coincidence.
    // A value that opens a collection is not a scalar, and the note above is
    // what makes splitting on the first byte free: the two scanners are
    // disjoint on it and neither moves the cursor when it fails.
    //
    // So each arm asks only the scanner that can still say yes. The bracket
    // case skips a scalar attempt that could only answer false; the other
    // case skips a collection attempt that could only answer false, because
    // `skip_collection` reads that same byte, finds it is neither bracket --
    // it does not skip space and has no other way in -- and stops at
    // `Illegal`. `a_collection_must_open_with_a_bracket` pins that.
    let found = match at(buf, i) {
        Some(b'{' | b'[') => skip_collection(buf, &mut i, max) == Validity::Valid,
        _ => skip_any_scalar(buf, &mut i, max),
    };

    if !found {
        return None;
    }

    *start = i;
    Some((value_start, i.saturating_sub(value_start)))
}

/// `nextKeyValuePair`: a `"key" : value` pair, and where it ends.
///
/// Returns `(key, key_length, value, value_length)`, with the key index
/// already moved past its opening quote and the length already shortened by
/// the two quotes.
fn next_key_value_pair(
    buf: &[u8],
    start: &mut usize,
    max: usize,
) -> Option<(usize, usize, usize, usize)> {
    let mut i = *start;
    let key_start = i;

    if !skip_string(buf, &mut i, max) {
        return None;
    }

    let key = key_start.saturating_add(1);
    let key_length = i.saturating_sub(key_start).saturating_sub(2);

    // `skip_space` stops on a colon, so when the colon is already here the
    // scan it replaces was going to be a no-op. One comparison finds that
    // out; only a key with space after it pays for the scan as well.
    if !(i < max && at(buf, i) == Some(b':')) {
        skip_space(buf, &mut i, max);

        if !(i < max && at(buf, i) == Some(b':')) {
            return None;
        }
    }

    i = i.saturating_add(1);
    skip_space(buf, &mut i, max);

    let (value, value_length) = next_value(buf, &mut i, max)?;

    *start = i;
    Some((key, key_length, value, value_length))
}

/// `objectSearch`: the value for a key, by walking the pairs in order.
///
/// The first match wins, so a document with duplicate keys answers with the
/// earlier one. That is the C's behaviour and it is worth knowing, because
/// ECMA-404 does not say which should win.
fn object_search(buf: &[u8], max: usize, query: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0usize;

    skip_space(buf, &mut i, max);

    if !(i < max && at(buf, i) == Some(b'{')) {
        return None;
    }

    i = i.saturating_add(1);
    skip_space(buf, &mut i, max);

    while i < max {
        let Some((key, key_length, value, value_length)) = next_key_value_pair(buf, &mut i, max)
        else {
            break;
        };

        if query.len() == key_length && buf.get(key..key.saturating_add(key_length)) == Some(query)
        {
            return Some((value, value_length));
        }

        if !skip_space_and_comma(buf, &mut i, max) {
            break;
        }
    }

    None
}

/// `arraySearch`: the value at an index, by counting values in order.
fn array_search(buf: &[u8], max: usize, query_index: u32) -> Option<(usize, usize)> {
    let mut i = 0usize;

    skip_space(buf, &mut i, max);

    if !(i < max && at(buf, i) == Some(b'[')) {
        return None;
    }

    i = i.saturating_add(1);
    skip_space(buf, &mut i, max);

    let mut current: u32 = 0;

    while i < max {
        let Some((value, value_length)) = next_value(buf, &mut i, max) else {
            break;
        };

        if current == query_index {
            return Some((value, value_length));
        }

        // The `u32::MAX` half of this is the C's overflow guard, and it is
        // why the loop stops rather than wrapping to index zero.
        if !skip_space_and_comma(buf, &mut i, max) || current == u32::MAX {
            break;
        }

        current = current.saturating_add(1);
    }

    None
}

/// `skipQueryPart`: everything up to the next separator or `[`.
///
/// Returns the part's length, or `None` if it would be empty — which is how
/// `.a` and `a..b` are refused.
fn skip_query_part(query: &[u8], start: &mut usize, max: usize) -> Option<usize> {
    let from = *start;
    let mut i = from;

    while i < max {
        match at(query, i) {
            Some(c) if c != QUERY_KEY_SEPARATOR && c != b'[' => i = i.saturating_add(1),
            _ => break,
        }
    }

    if i > from {
        *start = i;
        return Some(i.saturating_sub(from));
    }
    None
}

/// `multiSearch`: walk the query's parts, narrowing the buffer at each one.
///
/// The narrowing is the whole trick, and it is easy to transcribe wrongly:
/// `length` is both the size of the sub-buffer being searched **and** the
/// place the found value's length is written back to, so each part searches
/// only inside what the previous part found.
fn multi_search(buf: &[u8], query: &[u8]) -> Result<(usize, usize), NotFound> {
    let query_length = query.len();
    let mut i = 0usize;
    let mut start = 0usize;
    let mut length = buf.len();

    while i < query_length {
        // Each part searches the slice the last one found. This is always in
        // bounds: `start + length` only ever shrinks, because a found value
        // lies inside the buffer it was found in. The `else` is therefore
        // unreachable, and answering `Missing` rather than panicking is what
        // keeps the crate's no-panic property a property rather than a hope.
        let Some(sub) = buf.get(start..).and_then(|rest| rest.get(..length)) else {
            return Err(NotFound::Missing);
        };

        let found = if at(query, i) == Some(b'[') {
            i = i.saturating_add(1);

            let query_index = skip_digits_value(query, &mut i, query_length).unwrap_or(-1);

            if query_index < 0 || i >= query_length || at(query, i) != Some(b']') {
                return Err(NotFound::BadQuery);
            }

            i = i.saturating_add(1);

            // `query_index` is non-negative and at most MAX_INDEX_VALUE, so
            // this conversion cannot lose anything.
            array_search(sub, length, query_index.unsigned_abs())
        } else {
            let query_start = i;

            let Some(key_length) = skip_query_part(query, &mut i, query_length) else {
                return Err(NotFound::BadQuery);
            };

            // The C's one-liner for "an empty key part or a trailing
            // separator": landing exactly one byte short of the end means the
            // remaining byte is a separator with nothing after it.
            if i == query_length.saturating_sub(1) {
                return Err(NotFound::BadQuery);
            }

            let Some(key) = query.get(query_start..query_start.saturating_add(key_length)) else {
                return Err(NotFound::BadQuery);
            };

            object_search(sub, length, key)
        };

        let Some((value, value_length)) = found else {
            return Err(NotFound::Missing);
        };

        start = start.saturating_add(value);
        length = value_length;

        if i < query_length && at(query, i) == Some(QUERY_KEY_SEPARATOR) {
            i = i.saturating_add(1);
        }
    }

    Ok((start, length))
}

// ---- the public surface --------------------------------------------------

/// `JSON_SearchConst`: find a value by a dotted, bracketed path.
///
/// `a.b` is the key `b` of the object at key `a`; `a[2]` is the third element
/// of the array at key `a`; `[0].x` is the key `x` of the first element of a
/// top-level array. A key containing a `.` or a `[` cannot be reached — the
/// query grammar has no escape, in the C or here.
///
/// A [`Kind::String`] match has its quotes stripped, so the value is the text.
/// Every other kind is reported exactly as it appears, which means an object
/// or array comes back whole and can be searched again.
///
/// # Errors
///
/// [`NotFound::BadQuery`] for an empty buffer or query, a query part that is
/// empty, a trailing separator, or an index that is malformed or too large.
/// [`NotFound::Missing`] when the query simply is not there.
///
/// ```
/// use rusty_rtos_json_core::search::{search, Kind};
///
/// let doc = br#"{"a":{"b":[10,20,{"c":"hi"}]}}"#;
/// assert_eq!(search(doc, b"a.b[1]").unwrap().value, b"20");
/// assert_eq!(search(doc, b"a.b[2].c").unwrap().kind, Kind::String);
/// assert_eq!(search(doc, b"a.b[2].c").unwrap().value, b"hi");
/// ```
pub fn search<'a>(buf: &'a [u8], query: &[u8]) -> Result<Match<'a>, NotFound> {
    if buf.is_empty() || query.is_empty() {
        return Err(NotFound::BadQuery);
    }

    let (value, length) = multi_search(buf, query)?;

    // A found value always starts inside the buffer, so this is the same
    // unreachable-by-construction branch as the one in `multi_search`.
    let Some(first) = at(buf, value) else {
        return Err(NotFound::Missing);
    };

    let kind = kind_of(first);
    let (offset, length) = if kind == Kind::String {
        // Strip the surrounding quotes. A string value came from `skipString`
        // so it always carries both, and the subtraction cannot go negative.
        (value.saturating_add(1), length.saturating_sub(2))
    } else {
        (value, length)
    };

    let Some(slice) = buf.get(offset..).and_then(|rest| rest.get(..length)) else {
        return Err(NotFound::Missing);
    };

    Ok(Match {
        offset,
        value: slice,
        kind,
    })
}

/// `JSON_Iterate`: the next element of a collection, carrying its own cursor.
///
/// `start` marks where the collection begins and `next` where to look; both
/// are updated. Pass them in as zero to begin. [`pairs`] wraps this as a Rust
/// iterator and is what most callers want; this exists because it is the C's
/// shape, and because the differential compares the cursors themselves.
///
/// # Errors
///
/// [`Stopped::BadCursor`] for an empty buffer or a cursor outside it,
/// [`Stopped::NotACollection`] if the buffer does not begin with `[` or `{`,
/// and [`Stopped::Exhausted`] when there is nothing further.
pub fn iterate<'a>(
    buf: &'a [u8],
    start: &mut usize,
    next: &mut usize,
) -> Result<Pair<'a>, Stopped> {
    let max = buf.len();

    if max == 0 || *start >= max || *next > max {
        return Err(Stopped::BadCursor);
    }

    skip_space(buf, start, max);

    if *next <= *start {
        *next = start.saturating_add(1);
        skip_space(buf, next, max);
    }

    // `skipSpace` above can push `start` to the end, which is why this is
    // checked again rather than relying on the guard at the top.
    let Some(opener) = at(buf, *start) else {
        return Err(Stopped::Exhausted);
    };

    let (key, key_length, value, value_length) = match opener {
        b'[' => {
            let Some((value, value_length)) = next_value(buf, next, max) else {
                return Err(Stopped::Exhausted);
            };
            (0usize, 0usize, value, value_length)
        }
        b'{' => {
            let Some(found) = next_key_value_pair(buf, next, max) else {
                return Err(Stopped::Exhausted);
            };
            found
        }
        _ => return Err(Stopped::NotACollection),
    };

    let _ = skip_space_and_comma(buf, next, max);

    let Some(first) = at(buf, value) else {
        return Err(Stopped::Exhausted);
    };

    let kind = kind_of(first);
    let (offset, value_length) = if kind == Kind::String {
        (value.saturating_add(1), value_length.saturating_sub(2))
    } else {
        (value, value_length)
    };

    let Some(value_slice) = buf.get(offset..).and_then(|rest| rest.get(..value_length)) else {
        return Err(Stopped::Exhausted);
    };

    // Zero means "no key", which is the C's NULL. See `Pair::key`.
    let key_slice = if key == 0 {
        None
    } else {
        buf.get(key..).and_then(|rest| rest.get(..key_length))
    };

    Ok(Pair {
        key: key_slice,
        key_offset: if key_slice.is_some() { key } else { 0 },
        offset,
        value: value_slice,
        kind,
    })
}

/// The elements of a collection, as a Rust iterator.
///
/// This is [`iterate`] with its two cursors carried for you. It ends at the
/// first refusal of any kind, so a malformed document simply stops early
/// rather than reporting why — use [`iterate`] directly if the reason matters.
///
/// ```
/// use rusty_rtos_json_core::search::pairs;
///
/// let doc = br#"{"a":1,"b":"two","c":[3]}"#;
/// let keys: Vec<&[u8]> = pairs(doc).filter_map(|p| p.key).collect();
/// assert_eq!(keys, vec![&b"a"[..], b"b", b"c"]);
///
/// // An array yields values with no keys.
/// assert_eq!(pairs(b"[1,2,3]").count(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Pairs<'a> {
    buf: &'a [u8],
    start: usize,
    next: usize,
    done: bool,
}

/// Iterate the elements of a collection. See [`Pairs`].
#[must_use]
pub const fn pairs(buf: &[u8]) -> Pairs<'_> {
    Pairs {
        buf,
        start: 0,
        next: 0,
        done: false,
    }
}

impl<'a> Iterator for Pairs<'a> {
    type Item = Pair<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match iterate(self.buf, &mut self.start, &mut self.next) {
            Ok(pair) => Some(pair),
            Err(_) => {
                self.done = true;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Why [`next_value`] asks only one scanner per first byte.
    ///
    /// `skip_collection` reads the byte at the cursor and takes one of three
    /// arms: `{` or `[` opens a level, a closing bracket at depth zero is
    /// illegal, and everything else is illegal too. It does not skip leading
    /// space and it has no other way in -- so it can only answer `Valid` for
    /// a buffer that opens with a bracket, and `next_value`'s other arm has
    /// already matched both of those away.
    ///
    /// The implication only runs one way: replacing the opener of `[1,2]`
    /// with `{` does not make it valid. So this asserts the direction it
    /// relies on and no more.
    #[test]
    fn a_collection_must_open_with_a_bracket() {
        const DOCS: [&[u8]; 5] = [b"{}", b"[]", b"{\"a\":1}", b"[1,2]", b"[{}]"];

        for doc in DOCS {
            let length = doc.len();
            let mut buf = [0u8; 8];
            assert!(length <= buf.len(), "a document here outgrew the buffer");
            let Some(slot) = buf.get_mut(..length) else {
                unreachable!("the assertion above covers this")
            };
            slot.copy_from_slice(doc);

            for first in 0u8..=u8::MAX {
                let Some(head) = buf.first_mut() else {
                    unreachable!("the buffer is never empty")
                };
                *head = first;

                let Some(input) = buf.get(..length) else {
                    unreachable!("length came from a slice of this buffer")
                };

                let mut at = 0usize;
                if skip_collection(input, &mut at, length) == Validity::Valid {
                    assert!(
                        first == b'{' || first == b'[',
                        "a collection was valid opening with {first:#04x}"
                    );
                }
            }
        }
    }

    /// Why [`next_value`] may try its two scanners in either order.
    ///
    /// This started as a poison that did not fire. Swapping the order changed
    /// nothing anywhere in the differential, so the comment claiming the order
    /// mattered was wrong. Two facts make it free, and an accidental property
    /// nobody checks is one edit away from being false:
    ///
    /// 1. the scanners are **disjoint** -- no input satisfies both;
    /// 2. a scanner that fails **does not move the cursor**, so the fallback
    ///    starts exactly where the first attempt did.
    ///
    /// Lose either one and the order becomes load-bearing silently.
    #[test]
    fn the_two_scanners_are_disjoint_and_do_not_move_on_failure() {
        const INPUTS: [&[u8]; 18] = [
            b"",
            b" ",
            b"[",
            b"]",
            b"{",
            b"}",
            b"[]",
            b"{}",
            b"[1,2]",
            b"{\"a\":1}",
            b"42",
            b"-0.5e3",
            b"\"s\"",
            b"true",
            b"false",
            b"null",
            b"[1,2",
            b"{\"a\":",
        ];

        for input in INPUTS {
            let max = input.len();

            let mut scalar_at = 0usize;
            let scalar = skip_any_scalar(input, &mut scalar_at, max);

            let mut collection_at = 0usize;
            let collection = skip_collection(input, &mut collection_at, max) == Validity::Valid;

            assert!(
                !(scalar && collection),
                "both scanners claimed {input:?}, so the order WOULD matter"
            );

            if !scalar {
                assert_eq!(
                    scalar_at, 0,
                    "a failed scalar scan moved the cursor on {input:?}"
                );
            }
            if !collection {
                assert_eq!(
                    collection_at, 0,
                    "a failed collection scan moved the cursor on {input:?}"
                );
            }
        }
    }
}
