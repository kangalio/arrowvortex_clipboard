use crate::{Note, NoteKind};

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
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected EOF"),
            Self::MissingSignature => f.write_str("argument is not AV clipboard data"),
            Self::NonTrivial => f.write_str("non-trivial clipboard data is not supported yet"),
            Self::UnknownNoteType { note_type } => write!(f, "unknown note type {}", note_type),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

/// Convert `data` from AV clipboard format into bytes
fn decode_dwords_from_base85(data: &[u8]) -> impl Iterator<Item = u8> + '_ {
    // ArrowVortex groups bytes into 32bit ints and encodes them in base85 starting from ASCII 33.
    // Every 32bit int is represented by 5 base85 digits
    data.chunks(5).flat_map(|chunk| {
        let mut dword = 0;
        dword += 85_u32.pow(4) * chunk.get(0).map_or(85, |b| b - 33) as u32;
        dword += 85_u32.pow(3) * chunk.get(1).map_or(85, |b| b - 33) as u32;
        dword += 85_u32.pow(2) * chunk.get(2).map_or(85, |b| b - 33) as u32;
        dword += 85_u32.pow(1) * chunk.get(3).map_or(85, |b| b - 33) as u32;
        dword += 85_u32.pow(0) * chunk.get(4).map_or(85, |b| b - 33) as u32;
        core::array::IntoIter::new(dword.to_be_bytes())
    })
}

#[inline(never)]
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

/// Decodes a byte buffer into an iterator of [`Note`]
///
/// ```rust
/// use arrowvortex_clipboard::{Note, NoteKind};
///
/// let data = br#"ArrowVortex:notes:!!E9%!=T#H"!d"#;
/// let notes = arrowvortex_clipboard::decode(data)?.collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(&notes, &[
///     Note { row: 0, column: 0, kind: NoteKind::Tap },
///     Note { row: 12, column: 1, kind: NoteKind::Tap },
///     Note { row: 24, column: 2, kind: NoteKind::Tap },
///     Note { row: 36, column: 3, kind: NoteKind::Tap },
/// ]);
///
/// # Ok::<(), arrowvortex_clipboard::DecodeError>(())
/// ```
pub fn decode(
    data: &[u8],
) -> Result<impl Iterator<Item = Result<Note, DecodeError>> + '_, DecodeError> {
    let data = data
        .strip_prefix(b"ArrowVortex:notes:")
        .ok_or(DecodeError::MissingSignature)?;

    let mut data = decode_dwords_from_base85(data);

    if data.next().ok_or(DecodeError::UnexpectedEof)? != 0 {
        return Err(DecodeError::NonTrivial);
    }

    let size = decode_varint(&mut data)?;

    Ok((0..size).map(move |_| {
        let signifier = data.next().ok_or(DecodeError::UnexpectedEof)?;
        let is_tap = signifier & 0x80 == 0;
        let column = signifier & 0x7F;

        let row = decode_varint(&mut data)?;

        let note_kind = if is_tap {
            NoteKind::Tap
        } else {
            let end_row = decode_varint(&mut data)?;
            match data.next().ok_or(DecodeError::UnexpectedEof)? {
                0 => NoteKind::Hold { end_row },
                1 => NoteKind::Mine,
                2 => NoteKind::Roll { end_row },
                3 => NoteKind::Lift,
                4 => NoteKind::Fake,
                note_type => return Err(DecodeError::UnknownNoteType { note_type }),
            }
        };

        Ok(Note {
            row,
            column,
            kind: note_kind,
        })
    }))
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
