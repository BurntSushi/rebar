use std::{
    ffi::CStr,
    ptr::{self, NonNull},
};

use {
    anyhow::Context,
    libc::{c_char, c_int, c_void},
};

pub struct Regex {
    ure: NonNull<URegularExpression>,
    pattern: String,
}

// SAFETY: There don't appear to be any restrictions on sending an ICU regex
// to another thread. With that said, it exposes a mutable API (you actually
// set the text you want to search on the regex itself, lolwut), and so we
// only expose a mutable API. Therefore, it's trivially Sync, but uselessly
// so.
unsafe impl Send for Regex {}
unsafe impl Sync for Regex {}

impl std::fmt::Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Regex({:?})", self.pattern)
    }
}

impl Drop for Regex {
    fn drop(&mut self) {
        // SAFETY: By construction, the ICU regex is valid.
        unsafe { uregex_close_74(self.ure.as_ptr()) }
    }
}

impl Regex {
    /// Create a new regex from the given UTF-16 encoded pattern.
    pub fn new(pattern: &[u16], opts: Options) -> anyhow::Result<Regex> {
        let pattern_utf8 = String::from_utf16(pattern)
            .context("pattern is not valid UTF-16")?;
        let len = i32::try_from(pattern.len()).context("pattern too big")?;
        let mut ec = 0;
        // SAFETY: We use valid UTF-16, with a correct length. Also note that
        // the ICU docs specifically state that the pattern is copied and
        // saved, so we don't need to keep around a reference to it.
        let ure = unsafe {
            uregex_open_74(
                pattern.as_ptr(),
                len,
                opts.as_flags(),
                ptr::null_mut(),
                &mut ec,
            )
        };
        error_code_check(ec)?;
        let ure = NonNull::new(ure)
            .expect("non-null pointer for successfull uregex_open call");
        Ok(Regex { ure, pattern: pattern_utf8 })
    }

    /// Create a new matcher for searching the given haystack.
    pub fn matcher<'r, 'h>(
        &'r mut self,
        haystack: &'h [u16],
    ) -> anyhow::Result<Matcher<'r, 'h>> {
        Matcher::new(self, haystack)
    }

    /// Returns the total number of groups in this regex.
    ///
    /// This *includes* the implicit group corresponding to the overall match.
    pub fn group_len(&mut self) -> anyhow::Result<usize> {
        // SAFETY: There are no documented preconditions, but we know our regex
        // pointer is valid.
        let mut ec = 0;
        let len = unsafe { uregex_groupCount_74(self.ure.as_ptr(), &mut ec) };
        error_code_check(ec)?;
        usize::try_from(len + 1).context("invalid group count")
    }
}

/// A matcher for a regex.
///
/// `'r` is the lifetime of the regex and `'h` is the lifetime of the haystack
/// being searched.
///
/// This is used to encapsulate the lifetime behavior of a ICU regex. Namely,
/// an ICU regex works by permitting callers to set the haystack to search
/// on the regex itself. But it, of course, does not copy the haystack. As it
/// shouldn't. But this in turn means we need to be careful to only call
/// search routines when the haystack is valid.
///
/// This type lets us do exactly that. Namely, building this type corresponds
/// to a `setText` call with the haystack we want to search, while dropping it
/// result in another `setText` call that clears the haystack state from the
/// regex. This way, we ensure that we can never accidentally call the search
/// APIs with a haystack that has been freed. It also ensures we never let
/// an ICU regex hang on to a haystack that we have freed.
#[derive(Debug)]
pub struct Matcher<'r, 'h> {
    re: &'r mut Regex,
    // We never actually read from this haystack ourselves, but instead
    // put a marker here to represent the implicit borrow by the ICU regex.
    haystack: core::marker::PhantomData<&'h [u16]>,
}

impl<'r, 'h> Matcher<'r, 'h> {
    fn new(
        re: &'r mut Regex,
        haystack: &'h [u16],
    ) -> anyhow::Result<Matcher<'r, 'h>> {
        let len = i32::try_from(haystack.len()).context("haystack too big")?;
        let mut ec = 0;
        // SAFETY: There are no documented safety conditions, but we know our
        // regex and haystack pointers are valid, and so is our length. Also,
        // we call this again on Drop with a null pointer to clear the regex's
        // haystack state. That way, we don't leave an ICU regex hanging on to
        // a haystack that has potentially been freed.
        unsafe {
            uregex_setText_74(re.ure.as_ptr(), haystack.as_ptr(), len, &mut ec)
        }
        // There are no documented error conditions, but we check anyway.
        error_code_check(ec)?;
        let mut m = Matcher { re, haystack: core::marker::PhantomData };
        // It makes sense that ICU would do this automatically, but the
        // docs don't say if they do. So we do it...
        m.reset()?;
        Ok(m)
    }

    /// Looks for the next match, returning true if one was found and false
    /// if not. If there was a problem executing the search, then an error
    /// is returned.
    pub fn find(&mut self) -> anyhow::Result<bool> {
        let mut ec = 0;
        // SAFETY: There are no documented preconditions, but we know our regex
        // pointer is valid.
        let matched =
            unsafe { uregex_findNext_74(self.re.ure.as_ptr(), &mut ec) };
        error_code_check(ec)?;
        Ok(matched)
    }

    /// Returns a count of all successive matches.
    pub fn count(mut self) -> anyhow::Result<usize> {
        let mut n = 0;
        while self.find()? {
            n += 1;
        }
        Ok(n)
    }

    /// Returns the start offset of the given group, or None if the group
    /// did not participate in the match.
    ///
    /// If there is no match, then this returns an error.
    pub fn start(&mut self, group: usize) -> anyhow::Result<Option<usize>> {
        let group = i32::try_from(group).context("invalid group")?;
        let mut ec = 0;
        // SAFETY: There are no documented preconditions, but we know our regex
        // pointer is valid.
        let i =
            unsafe { uregex_start_74(self.re.ure.as_ptr(), group, &mut ec) };
        error_code_check(ec)?;
        if i == -1 {
            Ok(None)
        } else {
            usize::try_from(i).map(Some).context("invalid start offset")
        }
    }

    /// Returns the end offset of the given group, or None if the group did
    /// not participate in the match.
    ///
    /// If there is no match, then this returns an error.
    pub fn end(&mut self, group: usize) -> anyhow::Result<Option<usize>> {
        let group = i32::try_from(group).context("invalid group")?;
        let mut ec = 0;
        // SAFETY: There are no documented preconditions, but we know our regex
        // pointer is valid.
        let i = unsafe { uregex_end_74(self.re.ure.as_ptr(), group, &mut ec) };
        error_code_check(ec)?;
        if i == -1 {
            Ok(None)
        } else {
            usize::try_from(i).map(Some).context("invalid end offset")
        }
    }

    /// Resets the search state on this regex such that the search will start
    /// over at the beginning of the haystack.
    pub fn reset(&mut self) -> anyhow::Result<()> {
        let mut ec = 0;
        // SAFETY: There are no documented preconditions, but we know our regex
        // pointer is valid.
        unsafe {
            uregex_reset_74(self.re.ure.as_ptr(), 0, &mut ec);
        }
        error_code_check(ec)
    }
}

impl<'r, 'h> Drop for Matcher<'r, 'h> {
    fn drop(&mut self) {
        let mut ec = 0;
        // SAFETY: There are no documented safety conditions, but we know our
        // regex and haystack pointers are valid, and so is our length. Also,
        // the ICU docs explicitly state that for a zero length haystack, the
        // haystack pointer is never dereferenced. So it should be fine to pass
        // a null pointer here.
        //
        // Well, scratch that. I tried that and I got U_ILLEGAL_ARGUMENT_ERROR.
        // So why bother documenting that the pointer won't be dereferenced if
        // the length is 0?
        //
        // Instead, we just hand it a dummy pointer that is always empty
        // and always alive.
        unsafe {
            const EMPTY: &[u16] = &[];
            uregex_setText_74(self.re.ure.as_ptr(), EMPTY.as_ptr(), 0, &mut ec)
        }
        // There are no documented error conditions, but we check anyway.
        error_code_check(ec).unwrap();
    }
}

/// Options that can be passed to Regex::new to configure a subset
/// of ICU's regex knobs.
#[derive(Clone, Debug)]
pub struct Options {
    /// When enabled, ICU regex's case insensitive option is enabled.
    pub case_insensitive: bool,
}

impl Options {
    fn as_flags(&self) -> URegexpFlag {
        let mut flags = 0;
        if self.case_insensitive {
            flags |= U_REGEXP_FLAG_CASE_INSENSITIVE;
        }
        flags
    }
}

impl Default for Options {
    fn default() -> Options {
        Options { case_insensitive: false }
    }
}

/// If the error code given indicates a failure, then an error is returned
/// containing the name of the error.
fn error_code_check(ec: UErrorCode) -> anyhow::Result<()> {
    if ec <= 0 {
        return Ok(());
    }
    anyhow::bail!("{}", error_code_to_string(ec)?)
}

/// Converts the given error code to a string. If there was a problem getting
/// a string representation of the given code (perhaps if it is invalid), then
/// an error is returned.
fn error_code_to_string(ec: UErrorCode) -> anyhow::Result<String> {
    // SAFETY: There aren't any safety contracts on u_errorName, so we
    // assume it's fine to call with any value.
    let name = unsafe { u_errorName_74(ec) };
    anyhow::ensure!(
        !name.is_null(),
        "got null pointer from u_errorName, error code is probably wrong"
    );
    // SAFETY: u_errorName has no safety contracts, but we at least checked
    // that we didn't get a null pointer above.
    unsafe {
        Ok(CStr::from_ptr(name)
            .to_str()
            .context("invalid UTF-8 from u_errorName")?
            .to_string())
    }
}

// Below are our FFI declarations. We just hand-write what we need instead of
// trying to generate bindings for everything.
//
// And OH MY GOODNESS. Apparently the version of ICU is in every exposed
// symbol, so the code will actually need to bind to different symbols
// depending on which ICU version is installed. This seems insane. I guess
// it makes it possible to have multiple copies of ICU installed, but... just
// wow.
//
// I was unable to find absolutely anything about this in the docs.

type URegularExpression = c_void;
type UChar = u16;
type UErrorCode = c_int;
type URegexpFlag = u32;

const U_REGEXP_FLAG_CASE_INSENSITIVE: URegexpFlag = 2;

extern "C" {
    // Regex constructor and destructor.
    fn uregex_open_74(
        pattern: *const UChar,
        pattern_len: i32,
        flags: URegexpFlag,
        pe: *mut c_void, // we don't use this currently
        ec: *mut UErrorCode,
    ) -> *mut URegularExpression;
    fn uregex_close_74(re: *mut URegularExpression);

    // Sets the text to search on the regex object itself.
    // Yes, it is as weird as it sounds.
    fn uregex_setText_74(
        re: *mut URegularExpression,
        haystack: *const UChar,
        haystack_len: i32,
        ec: *mut UErrorCode,
    );

    // Finds the next match. Returns false if none were found.
    fn uregex_findNext_74(
        re: *mut URegularExpression,
        ec: *mut UErrorCode,
    ) -> bool;

    // Resets the given regex's internal search state.
    fn uregex_reset_74(
        re: *mut URegularExpression,
        index: i32,
        ec: *mut UErrorCode,
    );

    // Returns the start offset of the current match for
    // the given group, or an error if no match exists.
    fn uregex_start_74(
        re: *mut URegularExpression,
        group_num: i32,
        ec: *mut UErrorCode,
    ) -> i32;

    // Returns the end offset of the current match for
    // the given group, or an error if no match exists.
    fn uregex_end_74(
        re: *mut URegularExpression,
        group_num: i32,
        ec: *mut UErrorCode,
    ) -> i32;

    // Return the total number of capture groups in this
    // regex pattern.
    fn uregex_groupCount_74(
        re: *mut URegularExpression,
        ec: *mut UErrorCode,
    ) -> i32;

    // Utility routines.
    fn u_errorName_74(ec: UErrorCode) -> *const c_char;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn u_error_code_name() {
        let tostr = |code| error_code_to_string(code).unwrap();
        assert_eq!("U_ZERO_ERROR", tostr(0));
    }

    #[test]
    fn empty_not_infinite() {
        let mut re = Regex::new(&enc("$"), Options::default()).unwrap();
        let hay = enc("end\n");
        let mut m = re.matcher(&hay).unwrap();
        // Check that we won't get into an infinite loop with
        // zero-width matches. Some regex engines, *cough* Javascript
        // *cough*, don't handle this and will happily let you keep
        // spinning.
        assert!(m.find().unwrap());
        assert!(m.find().unwrap());
        assert!(!m.find().unwrap());
    }
}
