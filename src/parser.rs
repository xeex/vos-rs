use std::fs::File;
use std::io::{BufReader, SeekFrom};
use std::io::prelude::*;
use std::path::PathBuf;
use ghakuf::messages::*;
use ghakuf::reader::*;
use ghakuf::writer::*;
use std::path;

#[derive(Debug)]
struct NoteEvent {
    ticks: usize,
    ch: usize,
    note: usize,
    velocity: usize,
}

pub(crate) struct Parser {
    // path: PathBuf,
}

impl Parser {
    pub(crate) fn parse(path: PathBuf) -> std::io::Result<Parser> {

        /********************
         * READING VOS FILE *
         ********************/

        let f = File::open(path)?;
        let mut reader = BufReader::new(f);

        /*
         * HEADER
         */

        assert!(string_matches_hex(&mut reader, "020000000C000000566F736374656D702E74726B"));
        let midi_addr = dbg!(read_n_as_num(&mut reader, 4));

        // Quick detour to the MIDI section to save the initial MIDI file
        reader.seek(SeekFrom::Start(midi_addr as u64))?;
        ignore(&mut reader, 28);
        assert!(string_matches_hex(&mut reader, "564F534354454D502E6D6964"));
        let midi_len = dbg!(read_n_as_num(&mut reader, 4));
        let mut midi_buffer = vec![0; midi_len];
        reader.read(&mut midi_buffer)?;

        let mut midi_file = File::create("temp.mid")?;
        midi_file.write(&midi_buffer)?;

        let time_base;
        let write_messages: Vec<Message>;
        {
            let mut read_messages: Vec<Message> = Vec::new();
            let in_path = path::Path::new("temp.mid");
            let mut handler = HogeHandler {
                messages: &mut read_messages,
                time_base: 100,
            };
            let mut mid_reader = Reader::new(&mut handler, &in_path).unwrap();
            let _ = mid_reader.read();
            time_base = handler.time_base.clone();
            write_messages = read_messages.clone();
        }

        let mut writer = Writer::new();
        let out_path = path::Path::new("out.mid");
        writer.running_status(true);
        for message in &write_messages {
            println!("{:?}", message);
            writer.push(&message);
        }
        let mut additional_messages: Vec<Message> = Vec::new();

        // Back to header parsing
        reader.seek(SeekFrom::Start(24))?;
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
        let mut instruments = Vec::with_capacity(instrument_count);
        for _ in 0..instrument_count {
            assert!(string_matches_hex(&mut reader, "04"));
            let instrument = read_n_as_num(&mut reader, 4);
            instruments.push(instrument);
        }
        println!("Instruments: {:?}", instruments);

        assert!(string_matches_hex(&mut reader, "00"));
        let _level = 1 + read_byte(&mut reader);
        assert!(string_matches_hex(&mut reader, "0A004D69786564204D6F646500000000"));

        /*
         * NOTE DATA
         */

        for _ in 0..instrument_count {
            let mut raw_messages: Vec<NoteEvent> = Vec::new();

            let repeat = read_n_as_num(&mut reader, 4);
            assert!(string_matches_hex(&mut reader, "00"));
            for j in 0..repeat {
                let _mtime = read_n_as_num(&mut reader, 4);
                let _pitch = read_byte(&mut reader);
                let _track = read_byte(&mut reader);
                let _volume = read_byte(&mut reader);
                let _played = read_byte(&mut reader) == 1;
                ignore(&mut reader, 1);
                let _longnote = read_byte(&mut reader);
                let _soundlen = read_n_as_num(&mut reader, 4);
                ignore(&mut reader, 1);

                // One measure in VOS is 3072 ticks, whereas one measure in the MIDI is 4 times the base_time
                // (saved in the handler above)
                raw_messages.push(NoteEvent {
                    ticks: mtime_to_ticks(_mtime, time_base),
                    ch: _track,
                    note: _pitch,
                    velocity: _volume,
                });
                raw_messages.push(NoteEvent {
                    ticks: mtime_to_ticks(_mtime + _soundlen, time_base),
                    ch: _track,
                    note: _pitch,
                    velocity: 0,
                });


                //println!("\tMTIME: {} PITCH: {} TRACK: {} VOL: {} PLAYED: {} LN: {} SOUNDLEN: {}", _mtime, _pitch, _track, _volume, _played, _longnote, _soundlen);

                if j < repeat - 1 {
                    assert!(string_matches_hex(&mut reader, "00"));
                }
            }
            raw_messages.sort_by(|a, b| a.ticks.partial_cmp(&b.ticks).unwrap());
            for msg in raw_messages.iter() {
                println!("{:?}", msg);
            }

            // Write the new track to the midi file
            let track_ch = raw_messages[0].ch;
            additional_messages.push(Message::TrackChange);
            if track_ch < instruments.len() {
                additional_messages.push(Message::MidiEvent {
                    delta_time: 0,
                    event: MidiEvent::ProgramChange {
                        ch: track_ch as u8,
                        program: instruments[track_ch] as u8,
                }});
            }
            let mut last_ticks = 0;
            for msg in raw_messages.iter() {
                additional_messages.push(Message::MidiEvent {
                    delta_time: (msg.ticks - last_ticks) as u32,
                    event: MidiEvent::NoteOn {
                        ch: msg.ch as u8,
                        note: msg.note as u8,
                        velocity: msg.velocity as u8,
                    }
                });
                last_ticks = msg.ticks;
            }
            additional_messages.push(Message::MetaEvent {
                delta_time: 0,
                event: MetaEvent::EndOfTrack,
                data: Vec::new(),
            });

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
         * MIDI metadata
         */ 
        // This part has already been accounted for above.

        // Should be EOF

        /*******************
         * PROCESSING MIDI *
         *******************/

        for msg in &additional_messages {
            writer.push(&msg);
        }

        let _ = writer.write(&out_path);

        Ok(Parser {})
    }
}

/// Convert MTIME to MIDI ticks using time_base.
fn mtime_to_ticks(mtime: usize, time_base: u16) -> usize {
    ((mtime as f32/3072.0) * (4.0*time_base as f32)) as usize
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
    String::from_utf8(buffer).unwrap_or("Untitled".to_string()) // NOTE: should invoke EUC-KR encoding at some point
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


struct HogeHandler<'a> {
    messages: &'a mut Vec<Message>,
    time_base: u16,
}
impl<'a> Handler for HogeHandler<'a> {
    fn header(&mut self, format: u16, track: u16, time_base: u16) {
        println!(
            "SMF format: {}, track: {}, time base: {}",
            format, track, time_base
        );
        self.time_base = time_base;
    }
    fn meta_event(&mut self, delta_time: u32, event: &MetaEvent, data: &Vec<u8>) {
        println!(
            "delta time: {:>4}, Meta event: {}, data: {:?}",
            delta_time, event, data
        );
        self.messages.push(Message::MetaEvent {
            delta_time: delta_time,
            event: event.clone(),
            data: data.clone(),
        });
    }
    fn midi_event(&mut self, delta_time: u32, event: &MidiEvent) {
        println!("delta time: {:>4}, MIDI event: {}", delta_time, event,);
        self.messages.push(Message::MidiEvent {
            delta_time: delta_time,
            event: event.clone(),
        });
    }
    fn sys_ex_event(&mut self, delta_time: u32, event: &SysExEvent, data: &Vec<u8>) {
        println!(
            "delta time: {:>4}, System Exclusive Event: {}, data: {:?}",
            delta_time, event, data
        );
        self.messages.push(Message::SysExEvent {
            delta_time: delta_time,
            event: event.clone(),
            data: data.clone(),
        });
    }
    fn track_change(&mut self) {
        // Excepts first track change (from format chunk to data chunk)
        if self.messages.len() > 0 {
            println!("Track change occcurs!");
            self.messages.push(Message::TrackChange)
        }
    }
}