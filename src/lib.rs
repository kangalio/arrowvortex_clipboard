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
pub enum NoteKind<P> {
    /// Normal tap
    Tap,
    /// Hold note, spanning from this note's row up to end_row
    Hold {
        /// Where this hold note ends
        end_pos: P,
    },
    /// Mine note
    Mine,
    /// Roll note, spanning from this note's row up to end_row
    Roll {
        /// Where this roll note ends
        end_pos: P,
    },
    /// Lift note
    Lift,
    /// Fake note
    Fake,
}

impl<P> Default for NoteKind<P> {
    fn default() -> Self {
        Self::Tap
    }
}

/// Singular note
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Note<P> {
    /// Position of this note
    pub pos: P,
    /// Column of this note. Left-most column is 0
    pub column: u8,
    /// Type and type-specific data for this note
    pub kind: NoteKind<P>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TempoEventKind {
    Bpm {
        bpm: f64,
    },
    Stop {
        time: f64,
    },
    Delay {
        time: f64,
    },
    Warp {
        num_skipped_rows: u32,
    },
    TimeSignature {
        numerator: u32,
        denominator: u32,
    },
    Ticks {
        num_ticks: u32,
    },
    Combo {
        combo_multiplier: u32,
        miss_multiplier: u32,
    },
    Speed {
        ratio: f64,
        delay: f64,
        delay_is_time: bool,
    },
    Scroll {
        ratio: f64,
    },
    FakeSegment {
        num_fake_rows: u32,
    },
    Label {
        message_len: u64,
        // TODO: message string
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempoEvent {
    /// Row position of this tempo event
    pub pos: u32,
    /// Type and type-specific for this tempo event
    pub kind: TempoEventKind,
}
