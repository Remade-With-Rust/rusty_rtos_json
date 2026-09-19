//! `JSON_Validate`: coreJSON's strict ECMA-404 validator, over a byte slice.
//!
//! A transcription of `core_json.c` v3.3.1 (`cffa492`), which is a ladder of
//! `skip*` scanners each advancing a cursor. The shape is kept because the
//! shape IS the specification: coreJSON's acceptances and rejections fall out
//! of exactly where each scanner gives up, and a "tidier" parser would be a
//! different parser.
//!
//! Zero allocation, no recursion, `no_std`, `forbid(unsafe)`. Nesting is
//! bounded by an explicit stack of [`MAX_DEPTH`], as the C bounds it.
//!
//! # Measured before it was written
//!
//! coreJSON scores **100 %** on JSONTestSuite: 95 of 95 `y_` files accepted,
//! 188 of 188 `n_` files rejected. So "agree with the C" and "pass the suite"
//! are the same target here, which was worth establishing rather than
//! assuming — if the C had failed the suite anywhere, those two goals would
//! have pulled apart and one of them would have had to give.
//!
//! The 35 `i_` files, where the standard leaves the answer to the
//! implementation, are the interesting ones: coreJSON accepts 10 and rejects
//! 25. Being *a* JSON parser does not determine those; being *coreJSON* does,
//! and they are where a transcription drifts first.

/// `JSON_MAX_DEPTH`. The C's default, and the reason there is no recursion.
pub const MAX_DEPTH: usize = 32;

/// `JSONStatus_t`, for the subset [`validate`] can answer with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// `JSONSuccess`: valid and complete.
    Valid,
    /// `JSONIllegalDocument`: invalid or malformed.
    Illegal,
    /// `JSONMaxDepthExceeded`.
    MaxDepthExceeded,
    /// `JSONPartial`: valid so far, but the buffer ends mid-document.
    Partial,
    /// `JSONBadParameter`: an empty buffer.
    BadParameter,
}

// ---- the character classes, which are the C's macros ---------------------

/// `isspace_`, and it is JSON's whitespace rather than C's: space, tab,
/// newline and carriage return only. A vertical tab or form feed is NOT
/// whitespace to ECMA-404, and `isspace()` from `<ctype.h>` would say it was.
const fn is_space(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\n' || c == b'\r'
}

/// `isascii_`.
const fn is_ascii(c: u8) -> bool {
    c <= 0x7F
}

/// `iscntrl_`: an ASCII control character, which a string may not contain
/// unescaped.
const fn is_cntrl(c: u8) -> bool {
    is_ascii(c) && c < b' '
}

/// `isdigit_`.
const fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

const fn is_open_bracket(c: u8) -> bool {
    c == b'{' || c == b'['
}

const fn is_close_bracket(c: u8) -> bool {
    c == b'}' || c == b']'
}

/// `isMatchingBracket_`.
const fn is_matching_bracket(open: u8, close: u8) -> bool {
    (open == b'{' && close == b'}') || (open == b'[' && close == b']')
}

/// The byte at `i`, or `None` past the end. Every scanner reads through this,
/// so "past the end" is a value rather than a panic.
///
/// # This is defence in depth, and it was measured
///
/// Replacing the body with `Some(buf[i])` -- a panic on any read past the end
/// -- leaves all eleven tests passing, including 20,000 fuzz documents, every
/// truncation of six valid documents, every single-byte corruption of a
/// seventh, and all 318 corpus files. So no caller reads out of bounds today:
/// every scanner establishes `i < max` first.
///
/// The `Option` stays anyway, because that property holds only while every
/// caller is right, and a scanner edited later would lose it silently. The two
/// places where the bound is genuinely load-bearing are
/// [`skip_literal`]'s slice and the depth stack in [`validate`]; poisoning
/// either one panics immediately.
pub(crate) fn at(buf: &[u8], i: usize) -> Option<u8> {
    buf.get(i).copied()
}

// ---- the scanners --------------------------------------------------------

/// `skipSpace`.
pub(crate) fn skip_space(buf: &[u8], start: &mut usize, max: usize) {
    let mut i = *start;
    while i < max {
        match at(buf, i) {
            Some(c) if is_space(c) => i = i.saturating_add(1),
            _ => break,
        }
    }
    *start = i;
}

/// `countHighBits`.
const fn count_high_bits(c: u8) -> usize {
    let mut n = c;
    let mut i = 0usize;
    while (n & 0x80) != 0 {
        i = i.saturating_add(1);
        n = (n & 0x7F) << 1;
    }
    i
}

/// `shortestUTF8`: is this the shortest encoding of this value, and is the
/// value a legal scalar?
///
/// Rejecting an over-long encoding is a security property, not a nicety: the
/// classic exploit is smuggling an ASCII character past a filter as a
/// two-byte sequence.
const fn shortest_utf8(length: usize, value: u32) -> bool {
    let (min, max) = match length {
        2 => (1u32 << 7, (1u32 << 11) - 1),
        3 => (1u32 << 11, (1u32 << 16) - 1),
        _ => (1u32 << 16, 0x10_FFFF),
    };
    value >= min && value <= max && (value < 0xD800 || value > 0xDFFF)
}

/// `skipUTF8MultiByte`.
///
/// Out of line on purpose. Inlined, its leading-bit count and its
/// shortest-form table live inside [`skip_string`]'s frame and are paid for
/// by every string, including the ones that are entirely ASCII. See the note
/// on [`skip_escape`], which has to leave with it.
#[inline(never)]
fn skip_utf8_multibyte(buf: &[u8], start: &mut usize, max: usize) -> bool {
    let mut i = *start;
    let Some(first) = at(buf, i) else {
        return false;
    };
    // `> 0xC1` rejects the two over-long lead bytes; `< 0xF5` rejects
    // anything above U+10FFFF.
    if !(first > 0xC1 && first < 0xF5) {
        return false;
    }

    let bit_count = count_high_bits(first);
    let shift = 7usize.saturating_sub(bit_count);
    let mut value = u32::from(first) & ((1u32 << shift).saturating_sub(1));

    // The bit count is one greater than the number of continuation bytes.
    let mut j = bit_count.saturating_sub(1);
    while j > 0 {
        i = i.saturating_add(1);
        if i >= max {
            break;
        }
        let Some(c) = at(buf, i) else { break };
        // Continuation bytes must match 10xxxxxx.
        if (c & 0xC0) != 0x80 {
            break;
        }
        value = (value << 6) | u32::from(c & 0x3F);
        j = j.saturating_sub(1);
    }

    if j == 0 && shortest_utf8(bit_count, value) {
        *start = i.saturating_add(1);
        return true;
    }
    false
}

/// `skipUTF8`.
fn skip_utf8(buf: &[u8], start: &mut usize, max: usize) -> bool {
    if *start >= max {
        return false;
    }
    match at(buf, *start) {
        Some(c) if is_ascii(c) => {
            *start = start.saturating_add(1);
            true
        }
        Some(_) => skip_utf8_multibyte(buf, start, max),
        None => false,
    }
}

/// `NOT_A_HEX_CHAR`.
const NOT_A_HEX_CHAR: u8 = 0x10;

/// `hexToInt`.
const fn hex_to_int(c: u8) -> u8 {
    // Each arm's own range is what makes its arithmetic total: the
    // subtraction cannot go below zero and the sum cannot pass 15. The
    // saturating forms say so to the compiler as well as to the reader.
    match c {
        b'a'..=b'f' => c.saturating_sub(b'a').saturating_add(10),
        b'A'..=b'F' => c.saturating_sub(b'A').saturating_add(10),
        b'0'..=b'9' => c.saturating_sub(b'0'),
        _ => NOT_A_HEX_CHAR,
    }
}

/// `skipOneHexEscape`: `\uXXXX`.
fn skip_one_hex_escape(buf: &[u8], start: &mut usize, max: usize, out: &mut u16) -> bool {
    let mut i = *start;
    // `HEX_ESCAPE_LENGTH` is 6: the backslash, the `u`, and four digits.
    let end = i.saturating_add(6);
    let mut value = 0u16;

    // `end < max` and NOT `<=`, which is the C's. It means the escape must be
    // followed by at least one more byte -- and it always is, because a string
    // still needs its closing quote.
    if end > i
        && end < max
        && at(buf, i) == Some(b'\\')
        && at(buf, i.saturating_add(1)) == Some(b'u')
    {
        i = i.saturating_add(2);
        while i < end {
            let Some(c) = at(buf, i) else { break };
            let n = hex_to_int(c);
            if n == NOT_A_HEX_CHAR {
                break;
            }
            value = (value << 4) | u16::from(n);
            i = i.saturating_add(1);
        }
    }

    if i == end {
        *out = value;
        *start = i;
        return true;
    }
    false
}

const fn is_high_surrogate(x: u16) -> bool {
    x >= 0xD800 && x <= 0xDBFF
}

const fn is_low_surrogate(x: u16) -> bool {
    x >= 0xDC00 && x <= 0xDFFF
}

/// `skipHexEscape`: one escape, or a surrogate PAIR.
///
/// A high surrogate must be followed by a low one, and a lone low surrogate is
/// refused outright. That is stricter than "any four hex digits" and it is
/// where several `i_` files in JSONTestSuite land.
fn skip_hex_escape(buf: &[u8], start: &mut usize, max: usize) -> bool {
    let mut i = *start;
    let mut value = 0u16;
    let mut ret = false;

    if skip_one_hex_escape(buf, &mut i, max, &mut value) {
        if is_high_surrogate(value) {
            if skip_one_hex_escape(buf, &mut i, max, &mut value) && is_low_surrogate(value) {
                ret = true;
            }
        } else if is_low_surrogate(value) {
            // A premature low surrogate: refused.
        } else {
            ret = true;
        }
    }

    if ret {
        *start = i;
    }
    ret
}

/// `skipEscape`.
///
/// Out of line on purpose, and this is where most of it is: the `u` arm's
/// four hex digits and surrogate pairing needed callee-saved registers that
/// [`skip_string`] then had to push and pop on every call, escape or no
/// escape.
///
/// It only pays if [`skip_utf8_multibyte`] goes with it -- either one left
/// inline still claims the registers, so outlining this one alone takes half
/// as much off `search-ir` and puts nearly three times as much back on
/// `json-ir`. Keeping the two-byte escapes here and outlining only the rest
/// is worse than both, for the same reason: the match stays.
#[inline(never)]
fn skip_escape(buf: &[u8], start: &mut usize, max: usize) -> bool {
    let mut i = *start;
    let mut ret = false;

    // `i < max - 1`: an escape needs at least one byte after the backslash.
    if i < max.saturating_sub(1) && at(buf, i) == Some(b'\\') {
        match at(buf, i.saturating_add(1)) {
            Some(b'u') => ret = skip_hex_escape(buf, &mut i, max),
            Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                i = i.saturating_add(2);
                ret = true;
            }
            _ => {}
        }
    }

    if ret {
        *start = i;
    }
    ret
}

/// `skipString`.
pub(crate) fn skip_string(buf: &[u8], start: &mut usize, max: usize) -> bool {
    let mut i = *start;
    let mut ret = false;
    // `max` is a promise about the document; the buffer is the promise about
    // memory. Clamping one to the other says so once, and every read below is
    // then provably inside the slice -- where before each one carried its own
    // bounds check. Past the buffer the old loop stopped on a `None` read;
    // this one stops on the bound, at the same index.
    let max = max.min(buf.len());

    if i < max && at(buf, i) == Some(b'"') {
        i = i.saturating_add(1);
        while i < max {
            match at(buf, i) {
                Some(b'"') => {
                    ret = true;
                    i = i.saturating_add(1);
                    break;
                }
                Some(b'\\') => {
                    if !skip_escape(buf, &mut i, max) {
                        break;
                    }
                }
                // An unescaped control character is not allowed.
                Some(c) if is_cntrl(c) => break,
                Some(_) => {
                    if !skip_utf8(buf, &mut i, max) {
                        break;
                    }
                }
                None => break,
            }
        }
    }

    if ret {
        *start = i;
    }
    ret
}

/// `skipLiteral`.
fn skip_literal(buf: &[u8], start: &mut usize, max: usize, literal: &[u8]) -> bool {
    let from = *start;
    if from >= max || literal.len() > max.saturating_sub(from) {
        return false;
    }
    let end = from.saturating_add(literal.len());
    let Some(slice) = buf.get(from..end) else {
        return false;
    };
    if slice == literal {
        *start = end;
        return true;
    }
    false
}

/// `skipAnyLiteral`: `true`, `false` or `null`.
fn skip_any_literal(buf: &[u8], start: &mut usize, max: usize) -> bool {
    // The same dispatch as `skip_any_scalar`, one level down: the three
    // literals begin with three different bytes, so comparing against all of
    // them was two slice comparisons that could not match. `skip_literal`
    // writes `*start` only when the slice equals the literal, so declining to
    // run the two that cannot match changes nothing but the count.
    match at(buf, *start) {
        Some(b't') => skip_literal(buf, start, max, b"true"),
        Some(b'f') => skip_literal(buf, start, max, b"false"),
        Some(b'n') => skip_literal(buf, start, max, b"null"),
        _ => false,
    }
}

/// The largest value an array index may reach: `MAX_INDEX_VALUE`, which the
/// C's header defines as `0x7FFFFFF7`, or 2^31 - 9.
pub(crate) const MAX_INDEX_VALUE: i32 = 0x7FFF_FFF7;

/// `MAX_FACTOR`: the largest accumulator that can still take another digit.
const MAX_FACTOR: i32 = MAX_INDEX_VALUE / 10;

/// `skipDigits`, including the `outValue` half.
///
/// [`validate`] never asks for the value, but `multiSearch` reads an array
/// index with it, and the overflow behaviour is load-bearing there: once the
/// accumulator passes `MAX_FACTOR` the C latches it to **-1** and stops
/// accumulating, and a negative index is what makes `[99999999999999999999]`
/// a `BadParameter` rather than a wrapped-around lookup.
///
/// Returns `None` when there were no digits at all (the C's `false`), and
/// otherwise the accumulated value, which may be -1.
pub(crate) fn skip_digits_value(buf: &[u8], start: &mut usize, max: usize) -> Option<i32> {
    let from = *start;
    let mut i = from;
    let mut value: i32 = 0;

    while i < max {
        let Some(c) = at(buf, i) else { break };
        if !is_digit(c) {
            break;
        }

        if value > -1 {
            let n = i32::from(hex_to_int(c));
            value = if value <= MAX_FACTOR {
                value.saturating_mul(10).saturating_add(n)
            } else {
                -1
            };
        }

        i = i.saturating_add(1);
    }

    if i > from {
        *start = i;
        return Some(value);
    }
    None
}

/// `skipDigits` with `outValue == NULL`. The C skips only the accumulation,
/// never the scan, so this is the same walk with the answer thrown away.
fn skip_digits(buf: &[u8], start: &mut usize, max: usize) -> bool {
    skip_digits_value(buf, start, max).is_some()
}

/// `skipDecimals`. Note it advances `start` only when digits FOLLOW the dot,
/// so `1.` leaves the cursor on the dot and the document fails later.
fn skip_decimals(buf: &[u8], start: &mut usize, max: usize) {
    let mut i = *start;
    if i < max && at(buf, i) == Some(b'.') {
        i = i.saturating_add(1);
        if skip_digits(buf, &mut i, max) {
            *start = i;
        }
    }
}

/// `skipExponent`. Same shape as [`skip_decimals`]: a bare `e` does not
/// advance, so `1e` fails.
fn skip_exponent(buf: &[u8], start: &mut usize, max: usize) {
    let mut i = *start;
    if i < max && matches!(at(buf, i), Some(b'e' | b'E')) {
        i = i.saturating_add(1);
        if i < max && matches!(at(buf, i), Some(b'-' | b'+')) {
            i = i.saturating_add(1);
        }
        if skip_digits(buf, &mut i, max) {
            *start = i;
        }
    }
}

/// `skipNumber`.
fn skip_number(buf: &[u8], start: &mut usize, max: usize) -> bool {
    // Clamped once: past the buffer every read answers `None`, which fails
    // the comparison it feeds anyway -- and `skipDecimals` and `skipExponent`
    // fold into here, so one clamp covers all five of them.
    let max = max.min(buf.len());
    let mut i = *start;
    let mut ret = false;

    if i < max && at(buf, i) == Some(b'-') {
        i = i.saturating_add(1);
    }

    if i < max {
        // JSON disallows superfluous leading zeroes, so a leading zero is a
        // whole integer part on its own.
        if at(buf, i) == Some(b'0') {
            ret = true;
            i = i.saturating_add(1);
        } else {
            ret = skip_digits(buf, &mut i, max);
        }
    }

    if ret {
        skip_decimals(buf, &mut i, max);
        skip_exponent(buf, &mut i, max);
        *start = i;
    }
    ret
}

/// `skipAnyScalar`.
pub(crate) fn skip_any_scalar(buf: &[u8], start: &mut usize, max: usize) -> bool {
    // One dispatch on the first byte instead of up to three failed parses.
    // JSON is unambiguous at the first character, so at most one of these can
    // succeed: `skip_string` needs `"`, `skip_any_literal` needs `t`, `f` or
    // `n`, and `skip_number` needs `-` or a digit. The chain this replaces
    // tried the string, then all three literals, then the number -- so every
    // number paid for a failed string parse and three slice comparisons
    // before it began.
    //
    // Byte-identical by construction: each of the three writes `*start` only
    // on success, so a parse that cannot match leaves nothing behind, and not
    // running it is invisible to everything downstream.
    //
    // And no `*start >= max` guard: all three refuse that themselves --
    // `skip_string` tests `i < max`, `skip_literal` opens with `from >= max`,
    // `skip_number` guards both branches -- so a guard here is a second test
    // of something already tested.
    match at(buf, *start) {
        Some(b'"') => skip_string(buf, start, max),
        Some(b't' | b'f' | b'n') => skip_any_literal(buf, start, max),
        Some(b'-' | b'0'..=b'9') => skip_number(buf, start, max),
        _ => false,
    }
}

/// `skipSpaceAndComma`: true only when a comma is followed by more content.
///
/// A comma before a closing bracket answers false, which is what makes a
/// trailing comma illegal.
pub(crate) fn skip_space_and_comma(buf: &[u8], start: &mut usize, max: usize) -> bool {
    skip_space(buf, start, max);
    let mut i = *start;
    if i < max && at(buf, i) == Some(b',') {
        i = i.saturating_add(1);
        skip_space(buf, &mut i, max);
        if i < max && !at(buf, i).is_some_and(is_close_bracket) {
            *start = i;
            return true;
        }
    }
    false
}

/// `skipArrayScalars`.
fn skip_array_scalars(buf: &[u8], start: &mut usize, max: usize) -> bool {
    // Clamped once: past the buffer every read answers `None`, which
    // is what the loop below breaks on anyway.
    let max = max.min(buf.len());
    let mut i = *start;
    let mut ret = true;

    while i < max {
        // An element that opens a collection is not a scalar, so the scalar
        // attempt could only answer false and leave `i` where it is -- which
        // is the same state this break leaves. `skipCollection`'s stack is
        // what handles it, exactly as it does when the parse fails.
        if at(buf, i).is_some_and(is_open_bracket) {
            break;
        }
        if !skip_any_scalar(buf, &mut i, max) {
            break;
        }
        if !skip_space_and_comma(buf, &mut i, max) {
            // After a scalar there must be either a comma with more content,
            // or the closing bracket.
            if i >= max || at(buf, i) != Some(b']') {
                ret = false;
            }
            break;
        }
    }

    *start = i;
    ret
}

/// `skipObjectScalars`.
fn skip_object_scalars(buf: &[u8], start: &mut usize, max: usize) -> bool {
    // Clamped once: past the buffer every read answers `None`, which
    // is what the loop below breaks on anyway.
    let max = max.min(buf.len());
    let mut i = *start;
    let mut ret = true;

    while i < max {
        if !skip_string(buf, &mut i, max) {
            ret = false;
            break;
        }
        skip_space(buf, &mut i, max);
        if i < max && at(buf, i) != Some(b':') {
            ret = false;
            break;
        }
        i = i.saturating_add(1);
        skip_space(buf, &mut i, max);

        // A nested collection: hand back to `skipCollection`.
        if i < max && at(buf, i).is_some_and(is_open_bracket) {
            *start = i;
            break;
        }
        if !skip_any_scalar(buf, &mut i, max) {
            ret = false;
            break;
        }

        let comma = skip_space_and_comma(buf, &mut i, max);
        *start = i;
        if !comma {
            break;
        }
    }

    ret
}

/// `skipScalars`.
fn skip_scalars(buf: &[u8], start: &mut usize, max: usize, mode: u8) -> bool {
    // Clamped once: the guard below already says `i < max`, and clamping is
    // what makes that also say the read is inside the buffer. Past the buffer
    // the old `else` answered true; the clamped guard answers true at the
    // same index, by the line above it.
    let max = max.min(buf.len());
    skip_space(buf, start, max);
    let i = *start;
    if i >= max {
        return true;
    }
    let Some(c) = at(buf, i) else { return true };

    if mode == b'[' {
        if c != b']' {
            return skip_array_scalars(buf, start, max);
        }
    } else if c != b'}' {
        return skip_object_scalars(buf, start, max);
    }
    true
}

/// `skipCollection`: the explicit stack that replaces recursion.
pub(crate) fn skip_collection(buf: &[u8], start: &mut usize, max: usize) -> Validity {
    // Clamped once: past the buffer every read answers `None`, which
    // is what the loop below breaks on anyway.
    let max = max.min(buf.len());
    let mut ret = Validity::Partial;
    let mut stack = [0u8; MAX_DEPTH];
    // `int16_t depth = -1`, as an Option so there is no negative index.
    let mut depth: Option<usize> = None;
    let mut i = *start;

    while i < max {
        let Some(c) = at(buf, i) else { break };
        i = i.saturating_add(1);

        match c {
            b'{' | b'[' => {
                let next = match depth {
                    None => 0usize,
                    Some(d) => d.saturating_add(1),
                };
                if next >= MAX_DEPTH {
                    ret = Validity::MaxDepthExceeded;
                } else {
                    depth = Some(next);
                    if let Some(slot) = stack.get_mut(next) {
                        *slot = c;
                    }
                    if !skip_scalars(buf, &mut i, max, c) {
                        ret = Validity::Illegal;
                    }
                }
            }
            b'}' | b']' => {
                let d = depth.unwrap_or(0);
                let open = stack.get(d).copied().unwrap_or(0);
                // The nested case: close one level and carry on.
                if depth.is_some_and(|d| d > 0) && d < MAX_DEPTH && is_matching_bracket(open, c) {
                    let outer = d.saturating_sub(1);
                    depth = Some(outer);
                    let mode = stack.get(outer).copied().unwrap_or(0);
                    if skip_space_and_comma(buf, &mut i, max) {
                        if !skip_scalars(buf, &mut i, max, mode) {
                            ret = Validity::Illegal;
                        }
                    } else if i < max && !at(buf, i).is_some_and(|n| is_matching_bracket(mode, n)) {
                        // After closing a nested collection with no comma, the
                        // only legal next byte is the outer collection's own
                        // close.
                        ret = Validity::Illegal;
                    }
                } else {
                    ret = if depth == Some(0) && is_matching_bracket(open, c) {
                        Validity::Valid
                    } else {
                        Validity::Illegal
                    };
                }
            }
            _ => ret = Validity::Illegal,
        }

        if ret != Validity::Partial {
            break;
        }
    }

    if ret == Validity::Valid {
        *start = i;
    }
    ret
}

/// `JSON_Validate`: is this a complete, valid JSON document?
///
/// Strict ECMA-404, as coreJSON is strict: no trailing commas, no leading
/// zeroes, no unescaped control characters in strings, no over-long UTF-8, no
/// lone surrogates, and nesting no deeper than [`MAX_DEPTH`].
///
/// A **scalar at the top level is valid** — `42` and `"hi"` are documents —
/// which is ECMA-404 and not the older RFC 4627 rule that a document must be
/// an object or an array. The C makes that switchable with
/// `JSON_VALIDATE_COLLECTIONS_ONLY`; this follows the default.
#[must_use]
pub fn validate(buf: &[u8]) -> Validity {
    let max = buf.len();
    if max == 0 {
        // `JSONBadParameter` for `max == 0`. There is no null pointer to
        // report here, which is the one status this port cannot produce.
        return Validity::BadParameter;
    }

    let mut i = 0usize;
    skip_space(buf, &mut i, max);

    // A document that opens a collection cannot be a scalar, so the scalar
    // attempt is one the dispatch could only answer false to. The compiler
    // cannot know that; the grammar does.
    let mut ret = if at(buf, i).is_some_and(is_open_bracket) {
        skip_collection(buf, &mut i, max)
    } else if skip_any_scalar(buf, &mut i, max) {
        Validity::Valid
    } else {
        skip_collection(buf, &mut i, max)
    };

    if ret == Validity::Valid && i < max {
        skip_space(buf, &mut i, max);
        if i != max {
            ret = Validity::Illegal;
        }
    }

    ret
}

/// Whether [`validate`] accepts this document.
///
/// The convenience the differential and the test suite both want: coreJSON
/// distinguishes four kinds of "no", and a corpus only asks "yes or no".
#[must_use]
pub fn is_valid(buf: &[u8]) -> bool {
    validate(buf) == Validity::Valid
}
