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
let notes = match arrowvortex_clipboard::decode(data.as_bytes())? {
    arrowvortex_clipboard::DecodeResult::RowBasedNotes(notes) => {
        notes.collect::<Result<Vec<_>, _>>()?
    },
    _ => panic!("Unexpected data type"),
};
println!("{:?}", notes);

// Encode &[Note] into string
let mut buffer = String::new();
arrowvortex_clipboard::encode_row_based_notes(&mut buffer, &notes)?;
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

/// Tempo event type specific data
#[derive(Debug, Clone, PartialEq)]
pub enum TempoEventKind {
    /// Changes BPM (beats per minute)
    Bpm {
        /// BPM value
        bpm: f64,
    },
    /// Stops for a number of seconds
    Stop {
        /// Duration in seconds
        time: f64,
    },
    /// Delays for a number of seconds
    Delay {
        /// Duration in seconds
        time: f64,
    },
    /// Warps over a number of rows
    Warp {
        /// Length in rows
        num_skipped_rows: u32,
    },
    /// Changes time signature
    TimeSignature {
        /// Numerator of the time signature fraction
        numerator: u32,
        /// Denominator of the time signature fraction
        denominator: u32,
    },
    /// Changes number of ticks per beat
    Ticks {
        /// Number of ticks per beat
        num_ticks: u32,
    },
    /// Changes combo multiplier settings
    Combo {
        /// Combo multiplier
        combo_multiplier: u32,
        /// Miss multiplier
        miss_multiplier: u32,
    },
    /// Unknown
    Speed {
        /// Unknown
        ratio: f64,
        /// Unknown
        delay: f64,
        /// Unknown
        delay_is_time: bool,
    },
    /// Changes scroll speed
    Scroll {
        /// Scroll speed multiplier
        ratio: f64,
    },
    /// Converts all notes in the following rows into fakes
    FakeSegment {
        /// Length in rows
        num_fake_rows: u32,
    },
    /// Label with arbitrary content
    ///
    /// Only message length is stored currently, due to no_std restrictions
    Label {
        /// Message content
        message: Vec<u8>,
    },
}

/// Singular tempo event
#[derive(Debug, Clone, PartialEq)]
pub struct TempoEvent {
    /// Row position of this tempo event
    pub row: u32,
    /// Type and type-specific for this tempo event
    pub kind: TempoEventKind,
}
