use cat_audio::{backend::{client::{render::AudioRenderClient, AudioClient}, host::{DeviceEnumerator, Host}}, CoTaskMemFree, DeviceRole, OutputBuffer,SampleTransform, ShareMode, StreamFlags, WaveFormat};
use symphonia::{core::{audio::{SampleBuffer, SignalSpec}, codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions}, formats::{FormatOptions, FormatReader}, io::MediaSourceStream, meta::MetadataOptions}, default::{get_codecs, get_probe}};

use std::{ffi::c_void, fs::File, path::PathBuf, sync::{mpsc::{RecvTimeoutError, SyncSender}}, thread::JoinHandle, time::Duration, vec::IntoIter};

use crate::{event_queue::{Event, EventSender}, log, sample_rate_transformer::SampleRateConverter};



pub struct TrackReader {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32
}

impl TrackReader {
    pub fn new(file: File) -> Option<TrackReader> {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let Ok(probed) = get_probe().format(
            &Default::default(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        ) else { return None };

        let format = probed.format;

        let Some(track) = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL) else { return None };

        let track_id = track.id;

        let Ok(decoder) = get_codecs().make(&track.codec_params, &DecoderOptions::default()) else { return None };

        Some(
            Self {
                format,
                decoder,
                track_id: track_id
            }
        )
    }


    pub fn  next_packet(&mut self) -> Option<(SampleBuffer<f32>, SignalSpec)> {
        let mut l = 0;
        loop {
            l += 1;
            if l > 10{
                return None
            }

            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(_) => return None
            };

            while !self.format.metadata().is_latest() {
                // Pop the old head of the metadata queue.
                self.format.metadata().pop();

                // Consume the new metadata at the head of the metadata queue.
            }

            // If the packet does not belong to the selected track, skip over it.
            if packet.track_id() != self.track_id {
                continue
            }

            match self.decoder.decode(&packet) {
                Ok(audio_buffer) => {
                    let spec = *audio_buffer.spec();
                    let mut sample_buffer = SampleBuffer::<f32>::new(audio_buffer.capacity() as u64, spec);
                    sample_buffer.copy_interleaved_ref(audio_buffer);

                    l = 0;

                    return Some((sample_buffer, spec))
                }

                Err(_) => return None
            }
        }
    }
}


pub struct SoundObject {
    reader: TrackReader,
    buffer_size: u32,
    track_channels: usize,
    track_rate: u32,
    system_format: WaveFormat,
    next_samples: Option<Vec<f32>>,
    samples: SampleRateConverter<IntoIter<f32>>,
    playing: bool
}


impl SoundObject {
    pub fn new(file: File, system_format: &WaveFormat) -> Option<SoundObject> {
        let reader = TrackReader::new(file)?;

        let buffer_size = system_format.sample_rate * system_format.channels as u32 * 60;

<<<<<<< HEAD
        let track_channels = reader.decoder.codec_params().channels.unwrap_or_default().count();
        let track_rate = reader.decoder.codec_params().sample_rate.unwrap_or(48000);
=======
        let track_channels = reader.decoder.codec_params().channels.map(|c| c.count()).unwrap_or(1);
>>>>>>> 229415bbf1b3171add57320ae3d01552666a16b0

        let samples = SampleRateConverter::new(Vec::new().into_iter(), track_rate, system_format.sample_rate, system_format.channels as u32);

        Some (
            Self {
                reader,
                buffer_size,
                track_channels,
                track_rate,
                system_format: system_format.clone(),
                next_samples: None,
                samples,
                playing: true
            }
        )
    }
}


impl SoundObject {
    pub fn prepare(&mut self) {
        if let Some(buffer) = self.next_samples.as_mut() {
            if let Some((samples, spec)) = self.reader.next_packet() {
                self.track_channels = spec.channels.count();
                self.track_rate = spec.rate;
                buffer.extend_from_slice(samples.samples());
            }
        }
        else {
            let mut buffer = Vec::with_capacity(self.buffer_size as usize);
            if let Some((samples, spec)) = self.reader.next_packet() {
                self.track_channels = spec.channels.count();
                self.track_rate = spec.rate;
                buffer.extend_from_slice(samples.samples());
                self.next_samples = Some(buffer);
            }
        }
    }


    pub fn render(&mut self,output:&mut OutputBuffer, volume: f32) -> bool {
        match output {
            OutputBuffer::F32(buffer) => {
                for output in buffer.chunks_mut(self.system_format.channels as usize) {
                    if !self.playing {
                        for out_sample in output {
                            *out_sample = 0f32
                        }

                        continue
                    }

                    let mut sum_sample = 0f32;
                    for _ in 0..self.track_channels {
                        if let Some(sample) = self.samples.next() {
                            sum_sample += sample
                        }
                    }
                    sum_sample /= self.track_channels as f32;

                    for out_sample in output {
                        *out_sample = sum_sample * volume
                    }

                    if self.samples.input.len() == 0 {
                        if let Some(next_samples) = self.next_samples.take() {
                            self.samples = SampleRateConverter::new(next_samples.into_iter(), self.track_rate, self.system_format.sample_rate, self.track_channels as u32);
                        }
                        else {
                            self.playing = false
                        }
                    }
                }
            }

            OutputBuffer::I16(buffer) => {
                for output in buffer.chunks_mut(self.system_format.channels as usize) {
                    if !self.playing {
                        for out_sample in output {
                            *out_sample = 0i16
                        }

                        continue
                    }

                    let mut sum_sample = 0f32;
                    for _ in 0..self.track_channels {
                        if let Some(sample) = self.samples.next() {
                            sum_sample += sample
                        }
                    }
                    sum_sample /= self.track_channels as f32;

                    for out_sample in output {
                        *out_sample = sum_sample.to_i16(volume)
                    }

                    if self.samples.input.len() == 0 {
                        if let Some(next_samples) = self.next_samples.take() {
                            self.samples = SampleRateConverter::new(next_samples.into_iter(), self.reader.decoder.codec_params().sample_rate.unwrap_or(48000), self.system_format.sample_rate, self.track_channels as u32);
                        }
                        else {
                            self.playing = false
                        }
                    }
                }
            }

            OutputBuffer::U16(buffer) => {
                for output in buffer.chunks_mut(self.system_format.channels as usize) {
                    if !self.playing {
                        for out_sample in output {
                            *out_sample = 0u16
                        }

                        continue
                    }

                    let mut sum_sample = 0f32;
                    for _ in 0..self.track_channels {
                        if let Some(sample) = self.samples.next() {
                            sum_sample += sample
                        }
                    }
                    sum_sample /= self.track_channels as f32;

                    for out_sample in output {
                        *out_sample = sum_sample.to_u16(volume)
                    }

                    if self.samples.input.len() == 0 {
                        if let Some(next_samples) = self.next_samples.take() {
                            self.samples = SampleRateConverter::new(next_samples.into_iter(), self.reader.decoder.codec_params().sample_rate.unwrap_or(48000), self.system_format.sample_rate, self.track_channels as u32);
                        }
                        else {
                            self.playing = false
                        }
                    }
                }
            }
        }

        self.playing
    }
}



pub struct AudioEngine {
    _host: Host,
    _device_enumerator: DeviceEnumerator,
    client: AudioClient,
    pub wave_format: WaveFormat,
    render_client: AudioRenderClient
}

impl AudioEngine {
    pub fn new() -> AudioEngine {
        let host=Host::initialize().unwrap();

        let device_enumerator=DeviceEnumerator::new().unwrap();

        let device=device_enumerator.default_render_device(DeviceRole::Multimedia).unwrap();

        let client=device.activate().unwrap();

        let mut wave_format=client.get_mix_format().unwrap();

        let format=unsafe{*wave_format.ptr};

        let format=WaveFormat{
            sample_bits:format.wBitsPerSample,
            sample_rate:format.nSamplesPerSec,
            channels:format.nChannels
        };

        let flags=StreamFlags::new();
        client.initialize(ShareMode::Shared,flags,10_000_000,0,&mut wave_format,None).unwrap();

        unsafe{CoTaskMemFree(wave_format.ptr as *mut c_void);}

        let render_client=client.get_render_service().unwrap();

        client.start().unwrap();

        Self {
            _host: host,
            _device_enumerator: device_enumerator,
            wave_format: format,
            client,
            render_client
        }
    }


    fn render<F: FnMut(&mut [f32])>(&self, mut render: F) {
        let buffer_size=self.client.get_buffer_size().unwrap();

        let padding=self.client.get_current_padding().unwrap();
        let frames=buffer_size-padding;

        let buffer=self.render_client.get_buffer::<f32>(frames,self.wave_format.channels as u32).unwrap();

        render(buffer);

        self.render_client.release_buffer(frames,0).unwrap();
    }
}



pub enum AudioPlayerCommand {
    Play {
        path: PathBuf,
        volume: f32
    },


    Stop,

    Pause,

    Unpause,

    Volume(f32),

    Close
}


#[derive(Clone)]
pub struct AudioPlayerCommander {
    sender: SyncSender<AudioPlayerCommand>
}

impl AudioPlayerCommander {
    pub fn send(&self, command: AudioPlayerCommand) {
        let _ = self.sender.send(command);
    }
}


pub struct AudioPlayer {
    sender: SyncSender<AudioPlayerCommand>,
    thread: Option<JoinHandle<()>>
}


impl AudioPlayer {
    pub fn new(event_queue: EventSender) -> AudioPlayer {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<AudioPlayerCommand>(64);
    
        let thread = std::thread::spawn(move || {
            log!("AudioPlayer start");

            let audio_player = AudioEngine::new();

            let mut playing = true;
            let mut pause = false;
            let mut sound_volume = 0.5f32;
            let mut sound_object: Option::<SoundObject> = None;

            loop {
                match receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(command) => match command {
                        AudioPlayerCommand::Play{ path, volume} => {
                            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                            log!("Play {name}");

                            if let Ok(file) = std::fs::File::open(path.clone()) {
                                sound_object  = SoundObject::new(file, &audio_player.wave_format);

                                if sound_object.is_none() {
                                    event_queue.send(Event::Next);
                                    log!("Decode Error {name}");
                                }
                                else {
                                    playing = true;
                                    sound_volume = volume
                                }
                            }
                            else {
                                event_queue.send(Event::Next);
                                log!("File Error {name}");
                            }
                        }

                        AudioPlayerCommand::Stop => sound_object = None,

                        AudioPlayerCommand::Pause => pause = true,
                        AudioPlayerCommand::Unpause => pause = false,

                        AudioPlayerCommand::Volume(value) => sound_volume = value,

                        AudioPlayerCommand::Close => break
                    }

                    Err(RecvTimeoutError::Timeout) => {}

                    Err(RecvTimeoutError::Disconnected) => break
                }

                audio_player.render(|buffer| {
                    if !pause {
                        if let Some(sound) = sound_object.as_mut() {
                            sound.prepare();
                            playing = sound.render(&mut OutputBuffer::F32(buffer), sound_volume);

                            if !playing {
                                sound_object = None;
                                log!("Track end");
                                event_queue.send(Event::Next);
                            }
                        }
                    }

                    if !playing || pause {
                        for sample in buffer {
                            *sample = 0f32
                        }
                    }
                });
            }

            log!("AudioPlayer finish");
        });

        Self {
            sender,
            thread: Some(thread)
        }
    }


    pub fn commander(&self) -> AudioPlayerCommander {
        AudioPlayerCommander {
            sender: self.sender.clone()
        }
    }
}



impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.commander().send(AudioPlayerCommand::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}