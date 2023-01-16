use std::sync::{Arc, Mutex};
use fyrox_sound::algebra::Vector3;
use fyrox_sound::buffer::{DataSource, SoundBufferResource};
use fyrox_sound::context::SoundContext;
use fyrox_sound::futures::executor::block_on;
use fyrox_sound::pool::Handle;
use fyrox_sound::source::{SoundSource, SoundSourceBuilder};
use fyrox_sound::source::Status::Playing;
use gfx_maths::Vec3;
use halfbrown::HashMap;

pub struct AudioBackend {
    sounds: Arc<Mutex<HashMap<String, SoundBufferResource>>>,
    playing_sounds: Arc<Mutex<HashMap<String, Handle<SoundSource>>>>,
}

impl AudioBackend {
    pub fn new() -> Self {
        Self {
            sounds: Arc::new(Mutex::new(HashMap::new())),
            playing_sounds: Arc::new(Mutex::new(Default::default())),
        }
    }

    pub fn load_sound(&self, name: &str) {
        let mut sounds = self.sounds.lock().unwrap();
        // get full path
        let full_path = format!("base/snd/{}", name);
        // load sound
        let sound = SoundBufferResource::new_generic(block_on(DataSource::from_file(&full_path)).unwrap()).expect("failed to load sound");
        // insert into hashmap
        sounds.insert(name.to_string(), sound);
    }

    pub fn is_sound_loaded(&self, name: &str) -> bool {
        let sounds = self.sounds.lock().unwrap();
        sounds.contains_key(name)
    }

    pub fn is_sound_playing(&self, uuid: &str) -> bool {
        let playing_sounds = self.playing_sounds.lock().unwrap();
        playing_sounds.contains_key(uuid)
    }

    pub fn play_sound_with_uuid(&self, uuid: &str, name: &str, context: &SoundContext) {
        let sounds = self.sounds.lock().unwrap();
        let sound = sounds.get(name).unwrap();
        let mut playing_sounds = self.playing_sounds.lock().unwrap();
        let source = SoundSourceBuilder::new()
            .with_buffer(sound.clone())
            .with_looping(true)
            .with_status(Playing)
            .build().expect("failed to build sound source");
        let handle = context.state().add_source(source);
        playing_sounds.insert(uuid.to_string(), handle);
    }

    pub fn stop_sound_with_uuid(&self, uuid: &str, context: &SoundContext) {
        let mut playing_sounds = self.playing_sounds.lock().unwrap();
        let handle = playing_sounds.remove(uuid).unwrap();
        context.state().remove_source(handle);
    }

    pub fn set_sound_position(&self, uuid: &str, position: Vec3, context: &SoundContext) {
        let playing_sounds = self.playing_sounds.lock().unwrap();
        let handle = playing_sounds.get(uuid).unwrap();
        context.state().source_mut(*handle).set_position(Vector3::new(position.x, position.y, position.z));
    }

    pub fn update(&self, position: Vec3, forward: Vec3, up: Vec3, context: &SoundContext) {
        context.state().listener_mut().set_position(Vector3::new(position.x, position.y, position.z));
        context.state().listener_mut().set_orientation_rh(Vector3::new(forward.x, forward.y, forward.z), Vector3::new(up.x, up.y, up.z));
    }
}