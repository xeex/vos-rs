use std::fs::File;
use std::io::{BufReader, SeekFrom};
use std::io::prelude::*;
use std::path::PathBuf;

pub(crate) struct Parser {
    // path: PathBuf,
}

impl Parser {
    pub(crate) fn parse(path: PathBuf) -> std::io::Result<Parser> {
        let f = File::open(path)?;
        let mut reader = BufReader::new(f);

        /*
         * HEADER
         */

        assert!(string_matches_hex(&mut reader, "020000000C000000566F736374656D702E74726B"));
        let midi_addr = dbg!(read_n_as_num(&mut reader, 4));
        assert!(string_matches_hex(&mut reader, "564F53303232"));

        let _title = dbg!(read_variable_len(&mut reader));
        let _composer = dbg!(read_variable_len(&mut reader));
        let _sequencer = dbg!(read_variable_len(&mut reader));
        let _charter = dbg!(read_variable_len(&mut reader));
        let _genre = dbg!(read_variable_len(&mut reader));

        let _songtype = dbg!(read_byte(&mut reader));
        let _volume = dbg!(read_byte(&mut reader));

        ignore(&mut reader, 4);

        assert!(string_matches_hex(&mut reader, "00"));
        let _speed = dbg!(11 - read_byte(&mut reader));
        assert!(string_matches_hex(&mut reader, "0000"));

        ignore(&mut reader, 1);

        let _mtime = dbg!(read_n_as_num(&mut reader, 4));
        let _rtime = dbg!(read_n_as_num(&mut reader, 4));

        ignore(&mut reader, 1024);

        /*
         * DATA
         */

        let instrument_count = dbg!(read_n_as_num(&mut reader, 4));
        assert!(string_matches_hex(&mut reader, "01000000"));
        for i in 0..instrument_count {
            assert!(string_matches_hex(&mut reader, "04"));
            let instrument = read_n_as_num(&mut reader, 4);
            println!("Instrument {}: {}", i, instrument);
        }

        assert!(string_matches_hex(&mut reader, "00"));
        let _level = 1 + read_byte(&mut reader);
        assert!(string_matches_hex(&mut reader, "0A004D69786564204D6F646500000000"));

        /*
         * NOTE DATA
         */

        for _ in 0..instrument_count {
            let repeat = read_n_as_num(&mut reader, 4);
            assert!(string_matches_hex(&mut reader, "00"));
            for i in 0..repeat {
                let _mtime = read_n_as_num(&mut reader, 4);
                let _pitch = read_byte(&mut reader);
                let _track = read_byte(&mut reader);
                let _volume = read_byte(&mut reader);
                let _played = read_byte(&mut reader) == 1;
                ignore(&mut reader, 1);
                let _longnote = read_byte(&mut reader);
                let _soundlen = read_n_as_num(&mut reader, 4);
                ignore(&mut reader, 1);

                if i < repeat - 1 {
                    assert!(string_matches_hex(&mut reader, "00"));
                }
            }
        }

        // VOS022 Padding
        assert!(string_matches_hex(&mut reader, "00000000"));

        /*
         * PLAYING INFO
         */

        let num_notes = dbg!(read_n_as_num(&mut reader, 4));
        for _ in 0..num_notes {
            let _track = read_byte(&mut reader);
            let _tone = read_n_as_num(&mut reader, 4);
            let _key = read_byte(&mut reader);
        }

        /*
         * MIDI
         */

        reader.seek(SeekFrom::Start(midi_addr as u64))?;
        ignore(&mut reader, 28);
        assert!(string_matches_hex(&mut reader, "564F534354454D502E6D6964"));
        let midi_len = dbg!(read_n_as_num(&mut reader, 4));
        let mut midi_buffer = vec![0; midi_len];
        reader.read(&mut midi_buffer)?;

        // Should be EOF

        Ok(Parser {})
    }
}


/// Check if the next sequence of bytes is the same as `hex`.
/// C++: stringMatchesHex
fn string_matches_hex(file: &mut BufReader<File>, hex: &str) -> bool {
    let mut buffer = vec![0; hex.len() / 2];
    file.read(&mut buffer).unwrap();

    for i in (0..hex.len()).step_by(2) {
        let buffer_byte = buffer[i / 2] as u32;
        let hex_byte = u32::from_str_radix(&hex[i..i + 2], 16).unwrap();
        if buffer_byte != hex_byte { return false; }
    }

    true
}

/// Read the next `read_len` bytes in the file as chars, return as string.
/// C++: readN
pub fn read_n(file: &mut BufReader<File>, read_len: usize) -> String {
    let mut buffer = vec![0; read_len];
    file.read(&mut buffer).unwrap();
    String::from_utf8(buffer).unwrap() // NOTE: should invoke EUC-KR encoding at some point
}

/// Read the next `read_len` bytes in the file and return the result as int.
/// C++: readAsNum
fn read_n_as_num(file: &mut BufReader<File>, read_len: usize) -> usize {
    let mut buffer = vec![0; read_len];
    file.read(&mut buffer).unwrap();

    let mut result: usize = 0;
    for (i, num) in buffer.iter().enumerate() {
        result += *num as usize * 256_usize.pow(i as u32);
    }
    result
}

/// Read the next 2 bytes to determine the length, and read that many bytes.
/// Used primarily for VOS file headers.
/// C++: readVariableLen
fn read_variable_len(file: &mut BufReader<File>) -> String {
    let len: usize = read_n_as_num(file, 2);
    read_n(file, len)
}

/// Read a single byte and return it as int.
/// C++: readByte
fn read_byte(file: &mut BufReader<File>) -> usize {
    read_n_as_num(file, 1)
}

/// Ignore `len` number of bytes.
fn ignore(file: &mut BufReader<File>, len: usize) {
    file.seek(SeekFrom::Current(len as i64)).expect(&format!("Cannot ignore {} bytes", len));
}
