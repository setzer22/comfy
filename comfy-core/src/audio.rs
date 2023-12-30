use std::collections::hash_map::Entry;

use crate::*;

pub fn play_sound_ex(id: &str, params: PlaySoundParams) {
    play_sound_id_ex(sound_id(id), params);
}

pub fn play_sound_id_ex(sound: Sound, _params: PlaySoundParams) {
    play_sound_id(sound);
}

pub fn play_music_id_ex(sound: Sound, params: PlaySoundParams) {
    if params.looped {
        // TODO ??
        println!("looped music not supported yet");
    }
    play_sound_id(sound);
}

pub fn play_sound(id: &str) {
    play_sound_id(sound_id(id));
}

pub fn play_voice(id: &str) {
    play_sound_id(sound_id(id));
}

pub fn play_random_sound_ex(
    base_id: &str,
    amount: i32,
    settings: StaticSoundSettings,
) {
    let mut assets = ASSETS.borrow_mut();
    let id = format!("{}-{}", base_id, gen_range(1, amount + 1));

    AudioSystem::play_sound(
        &mut assets,
        sound_id(&id),
        Some(settings),
        AudioTrack::None,
    );
}

pub fn play_random_sound(base_id: &str, amount: i32) {
    let id = format!("{}-{}", base_id, gen_range(1, amount + 1));
    play_sound_id(sound_id(&id));
}

pub fn play_music(id: &str) {
    play_sound_id(sound_id(id));
}

pub fn play_sound_id(sound: Sound) {
    GLOBAL_STATE.borrow_mut().play_sound_queue.push(sound);
}

pub fn stop_sound(sound: &str) {
    stop_sound_id(sound_id(sound));
}

pub fn stop_sound_id(sound: Sound) {
    GLOBAL_STATE.borrow_mut().stop_sound_queue.push(sound);
}


#[derive(Copy, Clone, Debug, Default)]
pub struct PlaySoundParams {
    pub looped: bool,
}

pub struct PlaySoundCommand {
    pub sound: Sound,
    pub settings: StaticSoundSettings,
}

thread_local! {
    pub static AUDIO_SYSTEM: Lazy<RefCell<AudioSystem>> =
        Lazy::new(|| RefCell::new(AudioSystem::new()));
}

pub fn change_master_volume(change: f64) {
    AUDIO_SYSTEM.with(|audio| {
        if let Some(system) = audio.borrow_mut().system.as_mut() {
            system.master_volume =
                (system.master_volume + change).clamp(0.0, 1.0);

            system
                .master_track
                // .manager
                // .main_track()
                .set_volume(
                    Volume::Amplitude(system.master_volume),
                    kira::tween::Tween::default(),
                )
                .unwrap();
        }
    });
}

pub fn set_master_volume(value: f64) {
    AUDIO_SYSTEM.with(|audio| {
        if let Some(system) = audio.borrow_mut().system.as_mut() {
            system.master_volume = value.clamp(0.0, 1.0);

            system
                .master_track
                // .manager
                // .main_track()
                .set_volume(
                    Volume::Amplitude(system.master_volume),
                    kira::tween::Tween::default(),
                )
                .unwrap();
        }
    });
}

pub fn master_volume() -> f64 {
    AUDIO_SYSTEM.with(|audio| {
        if let Some(system) = audio.borrow_mut().system.as_ref() {
            system.master_volume
        } else {
            0.0
        }
    })
}


pub enum AudioTrack {
    None,
    Filter,
}

pub struct AudioSystemImpl {
    pub manager: AudioManager,
    pub master_track: TrackHandle,
    pub filter_track: TrackHandle,
    pub filter_handle: FilterHandle,

    pub master_volume: f64,
}

impl AudioSystemImpl {
    pub fn new(mut manager: AudioManager) -> Self {
        let mut builder = TrackBuilder::new();
        let filter_handle =
            builder.add_effect(FilterBuilder::new().cutoff(100.0));

        // builder.add_effect(ReverbBuilder::new().damping(0.8).feedback(0.9).mix(0.1));

        let filter_track =
            manager.add_sub_track(builder).expect("Failed to add filter track");

        let master_track = manager
            .add_sub_track(TrackBuilder::new())
            .expect("Failed to add master track");

        Self {
            manager,
            master_track,
            filter_track,
            filter_handle,
            master_volume: 1.0,
        }
    }

    pub fn play_sound(
        &mut self,
        assets: &mut Assets,
        sound: Sound,
        settings: Option<StaticSoundSettings>,
        track: AudioTrack,
        // ) -> Option<impl DerefMut<Target = StaticSoundHandle>> {
    ) {
        // TODO: get rid of excessive locking while processing a queue
        let sounds = assets.sounds.lock();

        if let Some(sound_data) = sounds.get(&sound).cloned() {
            match self.manager.play(sound_data) {
                Ok(handle) => {
                    match assets.sound_handles.entry(sound) {
                        Entry::Occupied(mut entry) => {
                            entry
                                .get_mut()
                                .stop(kira::tween::Tween::default())
                                .log_err();

                            entry.insert(handle);
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(handle);
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to play sound: {:?}", err);
                }
            }
        } else {
            error!("No sound data for {:?}", sound);
        }
    }

    pub fn process_sounds(&mut self) {}
}

pub struct AudioSystem {
    pub system: Option<AudioSystemImpl>,
}

impl AudioSystem {
    pub fn new() -> Self {
        // AudioManager::<kira::manager::backend::mock::MockBackend>::new(AudioManagerSettings::default())
        let manager =
            AudioManager::<kira::manager::backend::cpal::CpalBackend>::new(
                AudioManagerSettings::default(),
            )
            .map_err(|err| {
                error!("Failed to initialize audio manager: {:?}", err);
                err
            })
            .ok();

        let system = manager.map(AudioSystemImpl::new);

        Self { system }
    }

    pub fn process_sounds() {
        let _span = span!("process_sounds");

        let mut assets = ASSETS.borrow_mut();

        let stop_sound_queue =
            GLOBAL_STATE.borrow_mut().stop_sound_queue.drain(..).collect_vec();

        for sound in stop_sound_queue {
            match assets.sound_handles.entry(sound) {
                Entry::Occupied(mut entry) => {
                    entry
                        .get_mut()
                        .stop(kira::tween::Tween::default())
                        .log_err();
                    entry.remove();
                }
                Entry::Vacant(_) => {}
            }
        }

        let play_sound_queue =
            GLOBAL_STATE.borrow_mut().play_sound_queue.drain(..).collect_vec();

        for sound in play_sound_queue {
            AudioSystem::play_sound(&mut assets, sound, None, AudioTrack::None);
        }
    }

    pub fn play_sound(
        assets: &mut Assets,
        sound: Sound,
        settings: Option<StaticSoundSettings>,
        track: AudioTrack,
    ) {
        AUDIO_SYSTEM.with(|audio| {
            if let Some(system) = audio.borrow_mut().system.as_mut() {
                system.play_sound(assets, sound, settings, track);
            }
        });
    }
}
