use crate::{Note, NoteKind};

/// Error that may occur during [`encode()`]
#[derive(Debug)]
pub enum EncodeError {
    /// Error while writing data to the given output stream
    Write(core::fmt::Error),
    /// Input data was not sorted
    NotSorted,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncodeError::Write(w) => w.fmt(f),
            EncodeError::NotSorted => f.write_str("given notes are not sorted"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EncodeError::Write(w) => Some(w),
            EncodeError::NotSorted => None,
        }
    }
}

struct Base85Encoder<'a> {
    buffer: [u8; 4],
    buffer_i: usize,
    // I benchmarked: replacing this with static dispatch makes it SLOWER! 1.30ms -> 1.32ms
    writer: &'a mut dyn core::fmt::Write,
}

impl<'a> Base85Encoder<'a> {
    pub fn new(writer: &'a mut dyn core::fmt::Write) -> Self {
        Self {
            buffer: [0; 4],
            buffer_i: 0,
            writer,
        }
    }

    // #[inline(never)] slows this down
    pub fn write(&mut self, byte: u8) -> Result<(), EncodeError> {
        // Fill next buffer slot. If buffer isn't full yet, we're done
        self.buffer[self.buffer_i] = byte;
        self.buffer_i += 1;
        if self.buffer_i == 4 {
            self.flush_buffer()?;
        }
        Ok(())
    }

    // #[inline(never)] slows this down
    pub fn flush_buffer(&mut self) -> Result<(), EncodeError> {
        if self.buffer_i == 0 {
            return Ok(());
        }

        // Fill uninitialized bytes with zero
        self.buffer[self.buffer_i..].fill(0);

        let dword = u32::from_be_bytes(self.buffer);
        let buffer = [
            33 + ((dword / 85_u32.pow(4)) % 85) as u8,
            33 + ((dword / 85_u32.pow(3)) % 85) as u8,
            33 + ((dword / 85_u32.pow(2)) % 85) as u8,
            33 + ((dword / 85_u32.pow(1)) % 85) as u8,
            33 + ((dword / 85_u32.pow(0)) % 85) as u8,
        ];
        let buffer = &buffer[..(1 + self.buffer_i)];
        self.buffer_i = 0;
        self.writer
            .write_str(core::str::from_utf8(buffer).unwrap())
            .map_err(EncodeError::Write)
    }
}

// #[inline(never)] slows this down
fn encode_varint(writer: &mut Base85Encoder<'_>, mut n: u64) -> Result<(), EncodeError> {
    loop {
        let byte = n as u8 & 0x7F;
        n >>= 7;
        if n > 0 {
            writer.write(byte | 0x80)?;
        } else {
            writer.write(byte)?;
            break;
        }
    }

    Ok(())
}

/// Encodes a list of [`Note`]s into the given writer
///
/// Notes should be sorted by row and column to be pastable into ArrowVortex.
///
/// ```rust
/// use arrowvortex_clipboard::{Note, NoteKind};
///
/// let notes = &[
///     Note { row: 0, column: 0, kind: NoteKind::Tap },
///     Note { row: 12, column: 1, kind: NoteKind::Tap },
///     Note { row: 24, column: 2, kind: NoteKind::Tap },
///     Note { row: 36, column: 3, kind: NoteKind::Tap },
/// ];
///
/// let mut buffer = String::new();
/// arrowvortex_clipboard::encode(&mut buffer, notes)?;
/// assert_eq!(&buffer, r#"ArrowVortex:notes:!!E9%!=T#H"!d"#);
///
/// # Ok::<(), arrowvortex_clipboard::EncodeError>(())
/// ```
pub fn encode(writer: &mut dyn core::fmt::Write, notes: &[Note]) -> Result<(), EncodeError> {
    writer
        .write_str("ArrowVortex:notes:")
        .map_err(EncodeError::Write)?;
    let mut writer = Base85Encoder::new(writer);

    let is_sorted = notes
        .windows(2)
        .all(|w| (w[0].row, w[0].column) <= (w[1].row, w[1].column));
    if !is_sorted {
        return Err(EncodeError::NotSorted);
    }

    writer.write(0)?;
    encode_varint(&mut writer, notes.len() as u64)?;
    for note in notes {
        match note.kind {
            NoteKind::Tap => {
                writer.write(note.column & 0x7F)?;
                encode_varint(&mut writer, note.row)?;
            }
            NoteKind::Hold { end_row } | NoteKind::Roll { end_row } => {
                writer.write(note.column | 0x80)?;
                encode_varint(&mut writer, note.row)?;
                encode_varint(&mut writer, end_row)?;
            }
            NoteKind::Mine | NoteKind::Lift | NoteKind::Fake => {
                writer.write(note.column | 0x80)?;
                encode_varint(&mut writer, note.row)?;
                encode_varint(&mut writer, note.row)?;
            }
        }

        match note.kind {
            NoteKind::Tap => {}
            NoteKind::Hold { .. } => writer.write(0)?,
            NoteKind::Mine => writer.write(1)?,
            NoteKind::Roll { .. } => writer.write(2)?,
            NoteKind::Lift => writer.write(3)?,
            NoteKind::Fake => writer.write(4)?,
        };
    }

    writer.flush_buffer()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base85_encoder() {
        let mut buffer = String::new();
        let mut encoder = Base85Encoder::new(&mut buffer);
        for &byte in &[0xC9, 0xE8, 0xC9, 0x19, 0xDC, 0x2C, 0x7E, 0x0E] {
            encoder.write(byte).unwrap();
        }
        encoder.flush_buffer().unwrap();

        assert_eq!(buffer, "alphagamma");
    }

    #[test]
    fn test_encode_varint() {
        fn base85_encode(callback: impl FnOnce(&mut Base85Encoder<'_>)) -> String {
            let mut buffer = String::new();
            let mut encoder = Base85Encoder::new(&mut buffer);
            callback(&mut encoder);
            encoder.flush_buffer().unwrap();
            buffer
        }

        let result = base85_encode(|encoder| encode_varint(encoder, 58301).unwrap());
        let expected_result = base85_encode(|encoder| {
            for &byte in &[0xBD, 0xC7, 0x03] {
                encoder.write(byte).unwrap();
            }
        });

        assert_eq!(result, expected_result);
    }
}
