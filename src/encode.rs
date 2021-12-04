use crate::{Note, NoteKind, TempoEvent, TempoEventKind};

/// Error that may occur during any of the encoding functions
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

        if buffer == b"!!!!!" {
            self.writer.write_str("z").map_err(EncodeError::Write)
        } else {
            self.writer
                .write_str(core::str::from_utf8(buffer).unwrap())
                .map_err(EncodeError::Write)
        }
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

fn encode_f64(writer: &mut Base85Encoder<'_>, n: f64) -> Result<(), EncodeError> {
    for byte in n.to_le_bytes().iter().copied() {
        writer.write(byte)?;
    }

    Ok(())
}

fn encode_u32(writer: &mut Base85Encoder<'_>, n: u32) -> Result<(), EncodeError> {
    for byte in n.to_le_bytes().iter().copied() {
        writer.write(byte)?;
    }

    Ok(())
}

fn encode_notes<P: PartialOrd + Copy>(
    writer: &mut dyn core::fmt::Write,
    notes: &[Note<P>],
    time_based: bool,
    position_decode: impl Fn(&mut Base85Encoder<'_>, P) -> Result<(), EncodeError>,
) -> Result<(), EncodeError> {
    let is_sorted = notes
        .windows(2)
        .all(|w| (w[0].pos, w[0].column) <= (w[1].pos, w[1].column));
    if !is_sorted {
        return Err(EncodeError::NotSorted);
    }

    writer
        .write_str("ArrowVortex:notes:")
        .map_err(EncodeError::Write)?;
    let mut writer = Base85Encoder::new(writer);

    writer.write(time_based as u8)?;
    encode_varint(&mut writer, notes.len() as u64)?;
    for note in notes {
        match note.kind {
            NoteKind::Tap => {
                writer.write(note.column & 0x7F)?;
                position_decode(&mut writer, note.pos)?;
            }
            NoteKind::Hold { end_pos } | NoteKind::Roll { end_pos } => {
                writer.write(note.column | 0x80)?;
                position_decode(&mut writer, note.pos)?;
                position_decode(&mut writer, end_pos)?;
            }
            NoteKind::Mine | NoteKind::Lift | NoteKind::Fake => {
                writer.write(note.column | 0x80)?;
                position_decode(&mut writer, note.pos)?;
                position_decode(&mut writer, note.pos)?;
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

/// Encodes a list of row-based [`Note`]s into the given writer
///
/// Notes should be sorted by row and column to be pastable into ArrowVortex.
///
/// ```rust
/// use arrowvortex_clipboard::{Note, NoteKind};
///
/// let notes = &[
///     Note { pos: 0, column: 0, kind: NoteKind::Tap },
///     Note { pos: 12, column: 1, kind: NoteKind::Tap },
///     Note { pos: 24, column: 2, kind: NoteKind::Tap },
///     Note { pos: 36, column: 3, kind: NoteKind::Tap },
/// ];
///
/// let mut buffer = String::new();
/// arrowvortex_clipboard::encode_row_based_notes(&mut buffer, notes)?;
/// assert_eq!(&buffer, r#"ArrowVortex:notes:!!E9%!=T#H"!d"#);
///
/// # Ok::<(), arrowvortex_clipboard::EncodeError>(())
/// ```
pub fn encode_row_based_notes(
    writer: &mut dyn core::fmt::Write,
    notes: &[Note<u64>],
) -> Result<(), EncodeError> {
    encode_notes(writer, notes, false, encode_varint)
}

/// Encodes a list of time-based [`Note`]s into the given writer
///
/// Notes should be sorted by time and column to be pastable into ArrowVortex.
///
/// ```rust
/// use arrowvortex_clipboard::{Note, NoteKind};
///
/// let notes = &[
///     Note { pos: 0.0, column: 0, kind: NoteKind::Tap },
///     Note { pos: 0.25, column: 1, kind: NoteKind::Tap },
///     Note { pos: 0.5, column: 2, kind: NoteKind::Tap },
///     Note { pos: 0.75, column: 3, kind: NoteKind::Tap },
/// ];
///
/// let mut buffer = String::new();
/// arrowvortex_clipboard::encode_time_based_notes(&mut buffer, notes)?;
/// assert_eq!(&buffer, r#"ArrowVortex:notes:!<`B&z!!!!"z!!(A1!WW3#!!!#W56ClczkW]"#);
///
/// # Ok::<(), arrowvortex_clipboard::EncodeError>(())
/// ```
pub fn encode_time_based_notes(
    writer: &mut dyn core::fmt::Write,
    notes: &[Note<f64>],
) -> Result<(), EncodeError> {
    encode_notes(writer, notes, true, encode_f64)
}

fn tempo_event_kind(event: &TempoEventKind) -> u8 {
    match event {
        TempoEventKind::Bpm { .. } => 0,
        TempoEventKind::Stop { .. } => 1,
        TempoEventKind::Delay { .. } => 2,
        TempoEventKind::Warp { .. } => 3,
        TempoEventKind::TimeSignature { .. } => 4,
        TempoEventKind::Ticks { .. } => 5,
        TempoEventKind::Combo { .. } => 6,
        TempoEventKind::Speed { .. } => 7,
        TempoEventKind::Scroll { .. } => 8,
        TempoEventKind::FakeSegment { .. } => 9,
        TempoEventKind::Label { .. } => 10,
    }
}

fn group_by<'a, T, K: PartialEq>(
    mut slice: &'a [T],
    key: impl Fn(&T) -> K + 'a,
) -> impl Iterator<Item = (K, &[T])> + 'a {
    core::iter::from_fn(move || {
        let group_key = key(slice.get(0)?);
        let group_end = slice
            .iter()
            .position(|x| key(x) != group_key)
            .unwrap_or(slice.len());
        let (group, new_slice) = slice.split_at(group_end);
        slice = new_slice;
        Some((group_key, group))
    })
}

fn encode_single_tempo_event(
    writer: &mut Base85Encoder,
    event: &TempoEvent,
) -> Result<(), EncodeError> {
    encode_u32(writer, event.row)?;
    match &event.kind {
        &TempoEventKind::Bpm { bpm } => {
            encode_f64(writer, bpm)?;
        }
        &TempoEventKind::Stop { time } => {
            encode_f64(writer, time)?;
        }
        &TempoEventKind::Delay { time } => {
            encode_f64(writer, time)?;
        }
        &TempoEventKind::Warp { num_skipped_rows } => {
            encode_u32(writer, num_skipped_rows)?;
        }
        &TempoEventKind::TimeSignature {
            numerator,
            denominator,
        } => {
            encode_u32(writer, numerator)?;
            encode_u32(writer, denominator)?;
        }
        &TempoEventKind::Ticks { num_ticks } => {
            encode_u32(writer, num_ticks)?;
        }
        &TempoEventKind::Combo {
            combo_multiplier,
            miss_multiplier,
        } => {
            encode_u32(writer, combo_multiplier)?;
            encode_u32(writer, miss_multiplier)?;
        }
        &TempoEventKind::Speed {
            ratio,
            delay,
            delay_is_time,
        } => {
            encode_f64(writer, ratio)?;
            encode_f64(writer, delay)?;
            encode_u32(writer, delay_is_time as u32)?;
        }
        &TempoEventKind::Scroll { ratio } => {
            encode_f64(writer, ratio)?;
        }
        &TempoEventKind::FakeSegment { num_fake_rows } => {
            encode_u32(writer, num_fake_rows)?;
        }
        TempoEventKind::Label { message } => {
            encode_varint(writer, message.len() as u64)?;
            for &byte in message {
                writer.write(byte)?;
            }
        }
    }

    Ok(())
}

/// Encodes a list of [tempo events](TempoEvent) into the given writer
///
/// Events should be sorted by type and time to be pastable into ArrowVortex.
///
/// ```rust
/// use arrowvortex_clipboard::{TempoEvent, TempoEventKind};
///
/// let notes = &[
///     TempoEvent { row: 0, kind: TempoEventKind::Bpm { bpm: 120.0 } },
///     TempoEvent { row: 48, kind: TempoEventKind::Delay { time: 0.2 } },
///     TempoEvent { row: 96, kind: TempoEventKind::Warp { num_skipped_rows: 24 } },
///     TempoEvent { row: 144, kind: TempoEventKind::Scroll { ratio: 2.0 } },
/// ];
///
/// let mut buffer = String::new();
/// arrowvortex_clipboard::encode_tempo(&mut buffer, notes)?;
/// assert_eq!(&buffer, r#"ArrowVortex:tempo:!<<*"zz?9eMm0E;(QR[KS3R@2/]!<Z^0!!!i9!!!$*O8o7\z!!!!a!!"#);
///
/// # Ok::<(), arrowvortex_clipboard::EncodeError>(())
/// ```
pub fn encode_tempo(
    writer: &mut dyn core::fmt::Write,
    tempo_events: &[TempoEvent],
) -> Result<(), EncodeError> {
    let is_sorted = tempo_events.windows(2).all(|w| {
        (tempo_event_kind(&w[0].kind), w[0].row) <= (tempo_event_kind(&w[1].kind), w[1].row)
    });
    if !is_sorted {
        return Err(EncodeError::NotSorted);
    }

    writer
        .write_str("ArrowVortex:tempo:")
        .map_err(EncodeError::Write)?;
    let mut writer = Base85Encoder::new(writer);

    for (kind, events) in group_by(tempo_events, |ev| tempo_event_kind(&ev.kind)) {
        encode_varint(&mut writer, events.len() as u64)?;
        writer.write(kind)?;
        for event in events {
            encode_single_tempo_event(&mut writer, event)?;
        }
    }
    encode_varint(&mut writer, 0)?; // Empty count signifies end of tempo events list

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
