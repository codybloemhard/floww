use apres::{ MIDI, ApresError };
use apres::MIDIEvent::{ NoteOn, NoteOff, SetTempo };

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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
