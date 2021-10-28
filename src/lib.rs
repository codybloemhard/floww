use apres::{ MIDI, ApresError };
use apres::MIDIEvent::{ NoteOn, NoteOff, SetTempo };
use serde::{ Serialize, Deserialize };

use std::collections::{ HashMap };

// (id, time, note, vel)
pub type Point = (usize, f32, f32, f32);
pub type Floww = Vec<Point>;

pub fn midi_to_floww(midi: MIDI) -> Floww{
    let ppqn = midi.get_ppqn() as f32;
    let mut time_mult = 1.0; // 60bpm per default
    let mut floww = Vec::new();
    for track in midi.get_tracks(){
        let mut time = 0.0;
        for (tick, id) in track{
            time += tick as f32 / ppqn * time_mult;
            let ev = midi.get_event(id);
            if let Some(NoteOn(_, note, vel)) = ev {
                floww.push((note as usize, time, note as f32, vel as f32 / 127.0));
            }
            else if let Some(NoteOff(_, note, _)) = ev {
                floww.push((note as usize, time, note as f32, 0.0));
            }
            else if let Some(SetTempo(t)) = ev {
                time_mult = t as f32 / 1_000_000.0;
            }
        }
    }
    floww
}

pub fn read_floww_from_midi(path: &str) -> Result<Floww, ApresError>{
    match MIDI::from_path(path){
        Ok(midi) => { Ok(midi_to_floww(midi)) },
        Err(e) => Err(e),
    }
}

#[derive(Serialize, Deserialize)]
pub enum FlowwPacket{
    Msg(String),
    Track(String),
    Point(Point),
}

pub fn encode(batch: &[FlowwPacket]) -> Vec<u8>{
    bincode::serialize(batch).unwrap()
}

#[derive(Clone,Copy,Default)]
pub struct FlowwDecoder{
    current: usize,
}

impl FlowwDecoder{
    pub fn new() -> Self{
        Self{
            current: std::usize::MAX,
        }
    }

    pub fn decode(&mut self, flowws: &mut Vec<Floww>, map: &HashMap<String, usize>, data: &[u8]) -> Vec<String>{
        let mut messages = Vec::new();
        let batch: Vec<FlowwPacket> = if let Ok(res) = bincode::deserialize(data) { res }
        else { return messages; };
        for packet in batch{
            match packet{
                FlowwPacket::Msg(msg) => {
                    messages.push(msg);
                }
                FlowwPacket::Track(name) => {
                    self.current = if let Some(index) = map.get(&name){
                        *index
                    } else {
                        std::usize::MAX
                    };
                },
                FlowwPacket::Point(point) => {
                    if self.current == std::usize::MAX { continue; }
                    flowws[self.current].push(point);
                },
            }
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn it_works(){
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn encode_decode(){
        let mut tracks = vec![vec![], vec![], vec![]];
        let map: HashMap<String, usize> = [
            ("snare".to_string(), 0),
            ("kick".to_string(), 1),
            ("crash".to_string(), 2),
        ].iter().cloned().collect();

        let a = vec![
            FlowwPacket::Msg("beat".to_string()),
            FlowwPacket::Track("snare".to_string()),
            FlowwPacket::Point((0, 0.0, 0.25, 1.0)),
            FlowwPacket::Point((0, 0.0, 0.75, 1.0)),
            FlowwPacket::Track("kick".to_string()),
            FlowwPacket::Point((0, 0.0, 0.0 , 1.0)),
            FlowwPacket::Point((0, 0.0, 0.25, 1.0)),
            FlowwPacket::Point((0, 0.0, 0.5 , 1.0)),
            FlowwPacket::Point((0, 0.0, 0.75, 1.0)),
        ];

        let stream = encode(&a);
        let mut decoder = FlowwDecoder::new();
        let messages = decoder.decode(&mut tracks, &map, &stream);
        assert_eq!(messages, vec!["beat".to_string()]);
        assert_eq!(tracks[0], vec![(0, 0.0, 0.25, 1.0), (0, 0.0, 0.75, 1.0)]);
        assert_eq!(tracks[1], vec![(0, 0.0, 0.0, 1.0), (0, 0.0, 0.25, 1.0),
                                    (0, 0.0, 0.5, 1.0), (0, 0.0, 0.75, 1.0)]);
        let b = vec![
            FlowwPacket::Track("snare".to_string()),
            FlowwPacket::Point((0, 0.0, 1.0, 1.0)),
            FlowwPacket::Track("crash".to_string()),
            FlowwPacket::Point((0, 0.0, 1.0, 1.0)),
        ];
        let messages = decoder.decode(&mut tracks, &map, &encode(&b));
        assert_eq!(messages, Vec::<String>::new());
        assert_eq!(tracks[0], vec![(0, 0.0, 0.25, 1.0), (0, 0.0, 0.75, 1.0), (0, 0.0, 1.0, 1.0)]);
        assert_eq!(tracks[2], vec![(0, 0.0, 1.0, 1.0)]);
    }
}
