use core::fmt;

#[cfg(all(debug_assertions, feature = "alloc"))]
mod error_impl {
    use super::{Error, ErrorExt, ErrorKind};

    pub type ErrorInner = alloc::boxed::Box<Chained>;

    #[derive(Clone, Debug)]
    pub struct Chained {
        this: &'static ErrorKind,
        src: Option<Error>,
    }
    impl ErrorExt for Error {
        fn kind(&self) -> ErrorKind {
            *self.inner.this
        }
        fn source(&self) -> Option<&Error> {
            self.inner.src.as_ref()
        }
        fn chain_ctx(self, ctx: &'static ErrorKind) -> Error {
            Error {
                inner: Chained {
                    this: ctx,
                    src: Some(self),
                }
                .into(),
            }
        }
    }
    impl From<&'static ErrorKind> for Error {
        fn from(kind: &'static ErrorKind) -> Error {
            Error {
                inner: Chained {
                    this: kind,
                    src: None,
                }
                .into(),
            }
        }
    }
}

#[cfg(not(all(debug_assertions, feature = "alloc")))]
mod error_impl {
    use super::{Error, ErrorExt, ErrorKind};

    /// In release mode errors are just a thin pointer.
    pub type ErrorInner = &'static ErrorKind;
    impl ErrorExt for Error {
        fn kind(&self) -> ErrorKind {
            *self.inner
        }
        fn source(&self) -> Option<&Error> {
            None
        }
        fn chain_ctx(self, ctx: &'static ErrorKind) -> Error {
            Error { inner: ctx }
        }
    }
    impl From<&'static ErrorKind> for Error {
        fn from(inner: &'static ErrorKind) -> Error {
            Error { inner }
        }
    }
}

/// Represents an error parsing an SMF file or MIDI stream.
///
/// This type wraps an `ErrorKind` and includes backtrace and error chain data in debug mode.
/// In release mode it is a newtype wrapper around `ErrorKind`, so the `Fail::cause` method
/// always returns `None`.
///
/// For more information about the error policy used by `midly`, see
/// [`ErrorKind`](enum.ErrorKind.html).
#[derive(Clone)]
pub struct Error {
    inner: self::error_impl::ErrorInner,
}
impl Error {
    /// More information about the error itself.
    ///
    /// To traverse the causes of the error use the `Fail` trait instead.
    /// Note that error chains are only available in debug mode.
    pub fn kind(&self) -> ErrorKind {
        ErrorExt::kind(self)
    }

    /// The underlying cause for this error.
    ///
    /// Note that this method will always return `None` in release mode, since error chains
    /// are not tracked in release.
    pub fn source(&self) -> Option<&Error> {
        ErrorExt::source(self)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.kind(), f)
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.kind())?;
        let mut maybe_src = self.source();
        while let Some(src) = maybe_src {
            writeln!(f)?;
            write!(f, "  caused by: {}", src.kind())?;
            maybe_src = src.source();
        }
        Ok(())
    }
}
#[cfg(feature = "std")]
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

trait ErrorExt {
    fn kind(&self) -> ErrorKind;
    fn source(&self) -> Option<&Error>;
    fn chain_ctx(self, ctx: &'static ErrorKind) -> Error;
}

/// The type of error that occurred while parsing.
///
/// As a library consumer, detailed errors about what specific part of the MIDI spec was
/// violated are not very useful.
/// For this reason, errors are broadly categorized into 2 classes, and specific error info is
/// provided as a non-normative string literal.
#[derive(Copy, Clone, Debug)]
pub enum ErrorKind {
    /// Fatal errors while reading the file. It is likely that the file is not a MIDI file or
    /// is severely corrupted.
    ///
    /// This error cannot be ignored, as there is not enough data to continue parsing.
    /// No information about the file could be rescued.
    Invalid(&'static str),

    /// Non-fatal error, but the file is clearly corrupted.
    ///
    /// This kind of error is not emitted by default, only if the `strict` crate feature is
    /// enabled.
    ///
    /// Ignoring these errors can cause whole tracks to be skipped.
    Malformed(&'static str),
}
impl ErrorKind {
    /// Get the informative message on what exact part of the SMF format was not respected.
    pub fn message(&self) -> &'static str {
        match *self {
            ErrorKind::Invalid(msg) => msg,
            ErrorKind::Malformed(msg) => msg,
        }
    }
}
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorKind::Invalid(msg) => write!(f, "invalid midi: {}", msg),
            ErrorKind::Malformed(msg) => write!(f, "malformed midi: {}", msg),
        }
    }
}

macro_rules! err_invalid {
    ($msg:expr) => {{
        const ERR_KIND: &'static ErrorKind = &ErrorKind::Invalid($msg);
        ERR_KIND
    }};
}
macro_rules! err_malformed {
    ($msg:expr) => {{
        const ERR_KIND: &'static ErrorKind = &ErrorKind::Malformed($msg);
        ERR_KIND
    }};
}

pub trait ResultExt<T> {
    fn context(self, ctx: &'static ErrorKind) -> StdResult<T, Error>;
}
impl<T> ResultExt<T> for StdResult<T, Error> {
    fn context(self, ctx: &'static ErrorKind) -> StdResult<T, Error> {
        self.map_err(|err| err.chain_ctx(ctx))
    }
}
impl<T> ResultExt<T> for StdResult<T, &'static ErrorKind> {
    fn context(self, ctx: &'static ErrorKind) -> StdResult<T, Error> {
        self.map_err(|errkind| Error::from(errkind).chain_ctx(ctx))
    }
}

pub type Result<T> = StdResult<T, Error>;
pub use core::result::Result as StdResult;