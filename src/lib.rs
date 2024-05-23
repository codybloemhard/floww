use apres::{ MIDI, ApresError };
use apres::MIDIEvent::{ NoteOn, NoteOff, SetTempo };
use serde::{ Serialize, Deserialize };
use bincode::ErrorKind;
use fnrs::MutFunc;

use std::collections::{ HashMap };
use std::io::Read;

// (id, time, note, vel)
pub type Point = (usize, f32, f32, f32);
pub type Floww = Vec<Point>;

pub trait Timed{
    fn time(&self) -> f32;
    fn time_mut(&mut self) -> &mut f32;
    fn end(&self) -> f32;
    fn scale(&mut self, factor: f32);
}

impl Timed for Point{
    #[inline]
    fn time(&self) -> f32{
        self.1
    }

    #[inline]
    fn time_mut(&mut self) -> &mut f32{
        &mut self.1
    }

    #[inline]
    fn end(&self) -> f32{
        self.1
    }

    fn scale(&mut self, factor: f32){
        self.1 *= factor;
    }
}

pub trait TimedVec{
    fn sort(&mut self);
    fn shift_time(&mut self, t: f32);
    fn start_from_zero(&mut self);
    fn scale(&mut self, factor: f32);
    fn merge(&mut self, other: Self);
    fn fuse(&mut self, other: Self);

    fn sorted(self) -> Self;
    fn time_shifted(self, t: f32) -> Self;
    fn started_from_zero(self) -> Self;
    fn scaled(self, factor: f32) -> Self;
    fn merged(self, other: Self) -> Self;
    fn fused(self, other: Self) -> Self;
}

impl<T: Timed> TimedVec for Vec<T>{
    fn sort(&mut self){
        self.sort_by(|a,b| a.time().partial_cmp(&b.time()).unwrap());
    }

    fn shift_time(&mut self, t: f32){
        let begin_t = if let Some(p) = self.iter().next(){
            p.time()
        } else {
            return;
        };
        let shift = t.max(-begin_t);
        self.iter_mut().for_each(|p| *p.time_mut() += shift);
    }

    fn start_from_zero(&mut self){
        let begin_t = if let Some(p) = self.iter().next(){
            p.time()
        } else {
            return;
        };
        let shift = -begin_t;
        self.iter_mut().for_each(|p| *p.time_mut() += shift);
    }

    fn scale(&mut self, factor: f32){
        self.iter_mut().for_each(|p| p.scale(factor));
    }

    fn merge(&mut self, other: Self){
        self.extend(other);
        self.sort();
    }

    fn fuse(&mut self, other: Self){
        let l = self.len();
        if l == 0 {
            *self = other;
        } else {
            let last_t = self[l - 1].end();
            self.extend(other.time_shifted(last_t));
        }
    }

    fn sorted(mut self) -> Self{
        self.sort();
        self
    }

    fn time_shifted(mut self, t: f32) -> Self{
        self.shift_time(t);
        self
    }

    fn started_from_zero(mut self) -> Self{
        self.start_from_zero();
        self
    }

    fn scaled(mut self, factor: f32) -> Self{
        self.scale(factor);
        self
    }

    fn merged(mut self, other: Self) -> Self{
        self.merge(other);
        self
    }

    fn fused(mut self, other: Self) -> Self{
        self.fuse(other);
        self
    }
}

#[derive(Clone,Default)]
pub struct FlowwSheet{
    flowws: Vec<Floww>,
    names: Vec<String>,
    map: HashMap<String, usize>,
}

impl FlowwSheet{
    pub fn new() -> Self{
        Self::default()
    }

    pub fn add(&mut self, floww: Floww, name: String){
        let index = self.flowws.len();
        self.flowws.push(floww);
        self.map.insert(name.clone(), index);
        self.names.push(name);
    }

    pub fn get_floww_ref_by_name(&self, name: &str) -> &[Point]{
        if let Some(index) = self.map.get(name){
            &self.flowws[*index]
        } else {
            &[]
        }
    }

    pub fn get_names(&self) -> Vec<String>{
        self.names.clone()
    }

    pub fn reset(&mut self, name: &str, new: Floww) -> bool{
        if let Some(index) = self.map.get(name){
            self.flowws[*index] = new;
            true
        } else {
            false
        }
    }

    pub fn to_floww_packets(self) -> Vec<FlowwPacket>{
        let mut res = Vec::new();
        for (floww, name) in self.flowws.into_iter().zip(self.names.into_iter()){
            res.push(FlowwPacket::Track(name));
            for point in floww{
                res.push(FlowwPacket::Point(point));
            }
        }
        res
    }

    pub fn serialize(self) -> Result<Vec<u8>, Box<bincode::ErrorKind>>{
        let x = bincode::serialize(&self.flowws)?;
        let y = bincode::serialize(&self.names)?;
        Ok(x.conc(y))
    }
}

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

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum FlowwPacket{
    Msg(String),
    Track(String),
    Point(Point),
}

pub trait IntoFlowwPacket{
    fn into_packet(self) -> FlowwPacket;
}

impl IntoFlowwPacket for Point{
    fn into_packet(self) -> FlowwPacket{
        FlowwPacket::Point(self)
    }
}

pub trait IntoFlowwPackets{
    fn into_packets(self) -> Vec<FlowwPacket>;
}

impl<T: IntoFlowwPacket> IntoFlowwPackets for Vec<T>{
    fn into_packets(self) -> Vec<FlowwPacket>{
        self.into_iter().map(|i| i.into_packet()).collect::<Vec<_>>()
    }
}

pub trait Encodable{
    fn encode(&self) -> Vec<u8>;
    fn encoded(self) -> Vec<u8>;
}

impl Encodable for Vec<FlowwPacket>{
    fn encode(&self) -> Vec<u8>{
        bincode::serialize(self).unwrap()
    }

    fn encoded(self) -> Vec<u8>{
        self.encode()
    }
}

pub trait DecodeIntoFlowwPackets{
    fn decoded(self) -> Result<Vec<FlowwPacket>, Box<ErrorKind>>;
}

impl<T: Read> DecodeIntoFlowwPackets for T{
    fn decoded(self) -> Result<Vec<FlowwPacket>, Box<ErrorKind>>{
        bincode::deserialize_from(self)
    }
}

pub fn unpacket(flowws: &mut [Floww], map: &HashMap<String, usize>, packets: Vec<FlowwPacket>) -> Vec<String>{
    let mut current = 0;
    let mut messages = Vec::new();
    for packet in packets{
        match packet{
            FlowwPacket::Msg(msg) => {
                messages.push(msg);
            },
            FlowwPacket::Track(name) => {
                current = if let Some(index) = map.get(&name){
                    *index
                } else {
                    std::usize::MAX
                };
            },
            FlowwPacket::Point(point) => {
                if current == std::usize::MAX { continue; }
                if current >= flowws.len() { continue; }
                flowws[current].push(point);
            },
        }
    }
    messages
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
            FlowwPacket::Point((0, 1.25, 0.0, 1.0)),
            FlowwPacket::Point((0, 1.75, 0.0, 1.0)),
            FlowwPacket::Track("kick".to_string()),
            FlowwPacket::Point((0, 1.0, 0.0, 1.0)),
            FlowwPacket::Point((0, 1.25, 0.0, 1.0)),
            FlowwPacket::Point((0, 1.5, 0.0, 1.0)),
            FlowwPacket::Point((0, 1.75, 0.0, 1.0)),
        ];

        let messages = unpacket(&mut tracks, &map, a.encoded().decoded().unwrap());
        assert_eq!(messages, vec!["beat".to_string()]);
        assert_eq!(tracks[0], vec![(0, 1.25, 0.0, 1.0), (0, 1.75, 0.0, 1.0)]);
        assert_eq!(tracks[1], vec![(0, 1.0, 0.0, 1.0), (0, 1.25, 0.0, 1.0),
                                    (0, 1.5, 0.0, 1.0), (0, 1.75, 0.0, 1.0)]);
        let b = vec![
            FlowwPacket::Track("snare".to_string()),
            FlowwPacket::Point((0, 2.0, 0.0, 1.0)),
            FlowwPacket::Track("crash".to_string()),
            FlowwPacket::Point((0, 2.0, 0.0, 1.0)),
        ];
        let messages = unpacket(&mut tracks, &map, b.encoded().decoded().unwrap());
        assert_eq!(messages, Vec::<String>::new());
        assert_eq!(tracks[0], vec![(0, 1.25, 0.0, 1.0), (0, 1.75, 0.0, 1.0), (0, 2.0, 0.0, 1.0)]);
        assert_eq!(tracks[2], vec![(0, 2.0, 0.0, 1.0)]);
    }

    #[test]
    fn floww_ops(){
        let a = vec![(0, 1.0, 0.0, 0.0), (1, 0.0, 0.0, 0.0)];
        let b = a.sorted();
        assert_eq!(b, vec![(1, 0.0, 0.0, 0.0), (0, 1.0, 0.0, 0.0)]);
        let c = b.time_shifted(5.0);
        assert_eq!(c, vec![(1, 5.0, 0.0, 0.0), (0, 6.0, 0.0, 0.0)]);
        let d = c.time_shifted(-10.0);
        assert_eq!(d, vec![(1, 0.0, 0.0, 0.0), (0, 1.0, 0.0, 0.0)]);
        let e = d.time_shifted(456.0).started_from_zero();
        assert_eq!(e, vec![(1, 0.0, 0.0, 0.0), (0, 1.0, 0.0, 0.0)]);
        let f = e.merged(vec![(2, 0.5, 0.0, 0.0)]);
        assert_eq!(f, vec![(1, 0.0, 0.0, 0.0), (2, 0.5, 0.0, 0.0), (0, 1.0, 0.0, 0.0)]);
        let g = f.fused(vec![(3, 1.0, 0.0, 0.0)]);
        assert_eq!(g, vec![(1, 0.0, 0.0, 0.0), (2, 0.5, 0.0, 0.0),
                            (0, 1.0, 0.0, 0.0), (3, 2.0, 0.0, 0.0)]);
        let h = g.scaled(2.0);
        assert_eq!(h, vec![(1, 0.0, 0.0, 0.0), (2, 1.0, 0.0, 0.0),
                            (0, 2.0, 0.0, 0.0), (3, 4.0, 0.0, 0.0)]);
        let i = h.into_packets();
        assert_eq!(i, vec![
            FlowwPacket::Point((1, 0.0, 0.0, 0.0)),
            FlowwPacket::Point((2, 1.0, 0.0, 0.0)),
            FlowwPacket::Point((0, 2.0, 0.0, 0.0)),
            FlowwPacket::Point((3, 4.0, 0.0, 0.0)),
        ]);
        let j = i.encode().decoded().unwrap();
        assert_eq!(i, j);
    }
}
