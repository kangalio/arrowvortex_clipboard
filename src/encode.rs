use crate::{Note, NoteKind};

struct Base85Encoder<'a> {
    buffer: [u8; 4],
    buffer_i: usize,
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

    pub fn write(&mut self, byte: u8) -> core::fmt::Result {
        // Fill next buffer slot. If buffer isn't full yet, we're done
        self.buffer[self.buffer_i] = byte;
        self.buffer_i += 1;
        if self.buffer_i == 4 {
            self.flush_buffer()?;
        }
        Ok(())
    }

    pub fn flush_buffer(&mut self) -> core::fmt::Result {
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
        self.writer.write_str(core::str::from_utf8(buffer).unwrap())
    }
}

fn encode_varint(writer: &mut Base85Encoder<'_>, mut n: u64) -> core::fmt::Result {
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
/// # Ok::<(), core::fmt::Error>(())
/// ```
pub fn encode(mut writer: impl core::fmt::Write, notes: &[Note]) -> core::fmt::Result {
    writer.write_str("ArrowVortex:notes:")?;
    let mut writer = Base85Encoder::new(&mut writer);

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
