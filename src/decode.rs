use crate::{Note, NoteKind, TempoEvent, TempoEventKind};

/// Error in [`decode`] call
#[derive(Debug)]
pub enum DecodeError {
    /// Input ended unexpectedly
    UnexpectedEof,
    /// Input does not have the required ArrowVortex signature at the start
    MissingSignature,
    /// Input is of non-trivial type and cannot be decoded by this library
    NonTrivial,
    /// Input contained an unknown note type
    UnknownNoteType {
        /// The unknown note type integer that was encountered
        note_type: u8,
    },
    /// Input contained an unknown tempo event type
    UnknownTempoEventType {
        /// The unknown tempo event type integer that was encountered
        tempo_event_type: u8,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected EOF"),
            Self::MissingSignature => f.write_str("argument is not AV clipboard data"),
            Self::NonTrivial => f.write_str("non-trivial clipboard data is not supported yet"),
            Self::UnknownNoteType { note_type } => write!(f, "unknown note type {}", note_type),
            Self::UnknownTempoEventType { tempo_event_type } => {
                write!(f, "unknown tempo event type {}", tempo_event_type)
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Convert `data` from AV clipboard format into bytes
fn decode_dwords_from_base85(data: &[u8]) -> impl Iterator<Item = u8> + '_ {
    let mut data = data.iter().copied();

    // ArrowVortex groups bytes into 32bit ints and encodes them in base85 starting from ASCII 33.
    // Every 32bit int is represented by 5 base85 digits
    core::iter::from_fn(move || {
        let first_char = data.next()?;
        // 'z' is a shorthand for an entire zero chunk
        if first_char == b'z' {
            return Some([0, 0, 0, 0]);
        }

        let dword = 85_u32.pow(4) * (first_char - 33) as u32
            + 85_u32.pow(3) * data.next().map_or(85, |b| b - 33) as u32
            + 85_u32.pow(2) * data.next().map_or(85, |b| b - 33) as u32
            + 85_u32.pow(1) * data.next().map_or(85, |b| b - 33) as u32
            + 85_u32.pow(0) * data.next().map_or(85, |b| b - 33) as u32;
        Some(dword.to_be_bytes())
    })
    .flat_map(core::array::IntoIter::new)
}

#[inline(never)]
// TODO: return i32 instead?
fn decode_varint(data: &mut dyn Iterator<Item = u8>) -> Result<u64, DecodeError> {
    let mut result = 0;
    for i in 0.. {
        let byte = data.next().ok_or(DecodeError::UnexpectedEof)?;
        let is_last_byte = byte & 0x80 == 0;
        let varint_digit = byte & 0x7F;

        result |= (varint_digit as u64) << (7 * i);
        if is_last_byte {
            break;
        }
    }
    Ok(result)
}

fn decode_f64(data: &mut dyn Iterator<Item = u8>) -> Result<f64, DecodeError> {
    Ok(f64::from_le_bytes([
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
    ]))
}

fn decode_notes<'a, P: 'static>(
    mut data: impl Iterator<Item = u8> + 'a,
    position_decode: fn(&mut dyn Iterator<Item = u8>) -> Result<P, DecodeError>,
) -> Result<Vec<Note<P>>, DecodeError> {
    let size = decode_varint(&mut data)?;

    let notes = (0..size).map(move |_| {
        let first_byte = data.next().ok_or(DecodeError::UnexpectedEof)?;
        let is_tap = first_byte & 0x80 == 0;
        let column = first_byte & 0x7F;

        let pos = position_decode(&mut data)?;

        let note_kind = if is_tap {
            NoteKind::Tap
        } else {
            let end_pos = position_decode(&mut data)?;
            match data.next().ok_or(DecodeError::UnexpectedEof)? {
                0 => NoteKind::Hold { end_pos },
                1 => NoteKind::Mine,
                2 => NoteKind::Roll { end_pos },
                3 => NoteKind::Lift,
                4 => NoteKind::Fake,
                note_type => return Err(DecodeError::UnknownNoteType { note_type }),
            }
        };

        Ok(Note {
            pos,
            column,
            kind: note_kind,
        })
    });
    notes.collect()
}

fn decode_u32(data: &mut dyn Iterator<Item = u8>) -> Result<u32, DecodeError> {
    Ok(u32::from_le_bytes([
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
        data.next().ok_or(DecodeError::UnexpectedEof)?,
    ]))
}

fn decode_single_tempo_event(
    data: &mut dyn Iterator<Item = u8>,
    kind: u8,
) -> Result<TempoEvent, DecodeError> {
    let pos = decode_u32(data)?;
    let kind = match kind {
        0 => TempoEventKind::Bpm {
            bpm: decode_f64(data)?,
        },
        1 => TempoEventKind::Stop {
            time: decode_f64(data)?,
        },
        2 => TempoEventKind::Delay {
            time: decode_f64(data)?,
        },
        3 => TempoEventKind::Warp {
            num_skipped_rows: decode_u32(data)?,
        },
        4 => TempoEventKind::TimeSignature {
            numerator: decode_u32(data)?,
            denominator: decode_u32(data)?,
        },
        5 => TempoEventKind::Ticks {
            num_ticks: decode_u32(data)?,
        },
        6 => TempoEventKind::Combo {
            combo_multiplier: decode_u32(data)?,
            miss_multiplier: decode_u32(data)?,
        },
        7 => TempoEventKind::Speed {
            ratio: decode_f64(data)?,
            delay: decode_f64(data)?,
            delay_is_time: decode_u32(data)? != 0,
        },
        8 => TempoEventKind::Scroll {
            ratio: decode_f64(data)?,
        },
        9 => TempoEventKind::FakeSegment {
            num_fake_rows: decode_u32(data)?,
        },
        10 => {
            let message_len = decode_varint(data)?;
            let mut message = Vec::with_capacity(message_len as usize);
            for _ in 0..message_len {
                message.push(data.next().ok_or(DecodeError::UnexpectedEof)?);
            }
            TempoEventKind::Label { message }
        }
        other => {
            return Err(DecodeError::UnknownTempoEventType {
                tempo_event_type: other,
            })
        }
    };
    Ok(TempoEvent { row: pos, kind })
}

fn decode_tempo<'a>(
    mut data: impl Iterator<Item = u8> + 'a,
) -> Result<Vec<TempoEvent>, DecodeError> {
    let mut count = decode_varint(&mut data)?;
    let mut kind = None;

    core::iter::from_fn(move || {
        if count == 0 {
            return None;
        };

        if kind.is_none() {
            kind = Some(match data.next() {
                Some(x) => x,
                None => return Some(Err(DecodeError::UnexpectedEof)),
            });
        }
        let event = decode_single_tempo_event(&mut data, kind.unwrap());

        count -= 1;
        if count == 0 {
            count = match decode_varint(&mut data) {
                Ok(x) => x,
                Err(e) => return Some(Err(e)),
            };
            kind = None;
        }

        Some(event)
    })
    .collect()
}

/// Possible contents of ArrowVortex clipboard data. Returned by [`decode()`].
pub enum DecodeResult {
    /// Row based notes copy (most common)
    RowBasedNotes(Vec<Note<u64>>),
    /// Time based notes copy (if you enabled Time Based Copy in the menu)
    TimeBasedNotes(Vec<Note<f64>>),
    /// Tempo events copy
    TempoEvents(Vec<TempoEvent>),
}

/// Decodes a byte buffer into an iterator of [`Note`]
///
/// ```rust
/// use arrowvortex_clipboard::{Note, NoteKind};
///
/// let data = br#"ArrowVortex:notes:!!E9%!=T#H"!d"#;
///
/// let notes = match arrowvortex_clipboard::decode(data)? {
///     arrowvortex_clipboard::DecodeResult::RowBasedNotes(notes) => notes,
///     _ => panic!("Unexpected data type"),
/// };
///
/// assert_eq!(&notes, &[
///     Note { pos: 0, column: 0, kind: NoteKind::Tap },
///     Note { pos: 12, column: 1, kind: NoteKind::Tap },
///     Note { pos: 24, column: 2, kind: NoteKind::Tap },
///     Note { pos: 36, column: 3, kind: NoteKind::Tap },
/// ]);
///
/// # Ok::<(), arrowvortex_clipboard::DecodeError>(())
/// ```
pub fn decode(data: &[u8]) -> Result<DecodeResult, DecodeError> {
    let (data, is_tempo) = if let Some(data) = data.strip_prefix(b"ArrowVortex:notes:") {
        (data, false)
    } else if let Some(data) = data.strip_prefix(b"ArrowVortex:tempo:") {
        (data, true)
    } else {
        return Err(DecodeError::MissingSignature);
    };

    let mut data = decode_dwords_from_base85(data);

    Ok(if is_tempo {
        DecodeResult::TempoEvents(decode_tempo(data)?)
    } else {
        let is_time_based = data.next().ok_or(DecodeError::UnexpectedEof)? != 0;

        if is_time_based {
            DecodeResult::TimeBasedNotes(decode_notes(data, decode_f64)?)
        } else {
            DecodeResult::RowBasedNotes(decode_notes(data, decode_varint)?)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_dwords_from_base85() {
        let bytes = decode_dwords_from_base85(b"alphagamma").collect::<Vec<u8>>();
        assert_eq!(bytes, [0xC9, 0xE8, 0xC9, 0x19, 0xDC, 0x2C, 0x7E, 0x0E]);
    }

    #[test]
    fn test_decode_varint() {
        let bytes = [
            0xBD, 0xC7, 0x03, 0xF0, 0x0D, 0xBA, 0xAD, 0xF0, 0x0D, 0xBA, 0xAD,
        ];
        assert_eq!(decode_varint(&mut bytes.iter().copied()).unwrap(), 58301);
    }
}
