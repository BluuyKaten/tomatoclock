use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use crate::NOISE_DIR;

const CHANNEL_BUFFER: usize = 4;

pub struct StreamingSource {
    rx: Receiver<AudioChunk>,
    current: Vec<f32>,
    position: usize,
    sample_rate: u32,
    channels: u16,
}

enum AudioChunk {
    Metadata { sample_rate: u32, channels: u16 },
    Samples(Vec<f32>),
}

impl StreamingSource {
    pub fn new(path: String, is_builtin: bool) -> Option<Self> {
        let (tx, rx) = sync_channel::<AudioChunk>(CHANNEL_BUFFER);

        let resolved_path = if is_builtin {
            dioxus::asset_resolver::asset_path(format!("{}/{}", NOISE_DIR.to_string(), path)).ok()?
        } else {
            std::path::PathBuf::from(&path)
        };

        let path_str = resolved_path.to_string_lossy().to_string();
        std::thread::spawn(move || {
            loop {
                if !stream_decode(&path_str, &tx) {
                    break;
                }
            }
        });

        Some(Self {
            rx,
            current: Vec::new(),
            position: 0,
            sample_rate: 44100,
            channels: 2,
        })
    }
}

impl Iterator for StreamingSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.position >= self.current.len() {
            match self.rx.recv() {
                Ok(AudioChunk::Metadata { sample_rate, channels }) => {
                    self.sample_rate = sample_rate;
                    self.channels = channels;
                    self.position = 0;
                    self.current = Vec::new();
                    return self.next();
                }
                Ok(AudioChunk::Samples(chunk)) => {
                    self.current = chunk;
                    self.position = 0;
                }
                Err(_) => return None,
            }
        }
        let sample = self.current[self.position];
        self.position += 1;
        Some(sample)
    }
}

impl rodio::Source for StreamingSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<std::time::Duration> { None }
}

// 真正的流式解码：逐包解码，逐块发送
fn stream_decode(path: &str, tx: &SyncSender<AudioChunk>) -> bool {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    use symphonia::default::{get_codecs, get_probe};

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut probed = match get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default()) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let track_id = match probed.format.tracks().iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.channels.is_some())
    {
        Some(t) => t.id,
        None => return false,
    };

    let track = match probed.format.tracks().iter().find(|t| t.id == track_id) {
        Some(t) => t,
        None => return false,
    };

    let mut decoder = match get_codecs().make(&track.codec_params, &DecoderOptions::default()) {
        Ok(d) => d,
        Err(_) => return false,
    };

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    // 发送元数据
    if tx.send(AudioChunk::Metadata { sample_rate, channels }).is_err() {
        return false;
    }

    let mut samples = Vec::new();
    loop {
        let packet = match probed.format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let spec = *decoded.spec();
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buf.copy_interleaved_ref(decoded);
        samples.extend_from_slice(buf.samples());

        // 积累一定量后发送
        if samples.len() >= 8192 {
            let chunk = std::mem::take(&mut samples);
            if tx.send(AudioChunk::Samples(chunk)).is_err() {
                return false;
            }
        }
    }

    // 发送剩余的采样
    if !samples.is_empty() {
        if tx.send(AudioChunk::Samples(samples)).is_err() {
            return false;
        }
    }

    true
}
