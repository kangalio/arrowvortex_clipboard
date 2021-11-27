#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![warn(missing_docs)]

/*!
Small library for encoding and decoding [ArrowVortex](https://arrowvortex.ddrnl.com/) clipboard
data. Ported from [av-clipboard-lib](https://github.com/DeltaEpsilon7787/av-clipboard-lib), a Python
libary by DeltaEpsilon.

Main credit goes to DeltaEpsilon for reverse-engineering ArrowVortex' clipboard functions and
implementing the first ArrowVortex clipboard library.

This library is no_std-compatible if you opt-out of the `std` feature. The `std` feature includes
an [`std::error::Error`] implementation for [`DecodeError`] and [`EncodeError`].

```rust
// EtternaOnline noteskin template pattern (https://etternaonline.com/noteskins)
let data = r#"ArrowVortex:notes:!"8i-K)chjJHuM^!#P_Z![IjrJi#:bJ2UO3!BC3L"%E"#;

// Decode string into Vec<Note>
let notes = arrowvortex_clipboard::decode(data.as_bytes())?
    .collect::<Result<Vec<_>, _>>()?;
println!("{:?}", notes);

// Encode &[Note] into string
let mut buffer = String::new();
arrowvortex_clipboard::encode(&mut buffer, &notes)?;
println!("{}", buffer);

// Verify that string stayed identical after roundtrip
assert_eq!(data, buffer);
# Ok::<(), Box<dyn std::error::Error>>(())
```
*/

mod decode;
pub use decode::*;

mod encode;
pub use encode::*;

/// Note-type specific data
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoteKind {
    /// Normal tap
    Tap,
    /// Hold note, spanning from this note's row up to end_row
    Hold {
        /// Where this hold note ends
        end_row: u64,
    },
    /// Mine note
    Mine,
    /// Roll note, spanning from this note's row up to end_row
    Roll {
        /// Where this roll note ends
        end_row: u64,
    },
    /// Lift note
    Lift,
    /// Fake note
    Fake,
}

impl Default for NoteKind {
    fn default() -> Self {
        Self::Tap
    }
}

/// Singular note
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Note {
    /// Noterow of this note
    pub row: u64,
    /// Column of this note. Left-most column is 0
    pub column: u8,
    /// Type of this note and type-specific data
    pub kind: NoteKind,
}
