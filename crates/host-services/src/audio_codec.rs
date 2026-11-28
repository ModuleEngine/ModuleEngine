//! Bounded decoding of portable cue assets into Kira frames.
use kira::{dsp::Frame, sound::static_sound::StaticSoundData};
use std::{
    fs::File,
    io::{Cursor, Read, Seek},
    path::Path,
};

pub(crate) const MAX_FRAMES: usize = 32 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
// +24 dBFS relative to a normalized sample of 1.0.
const MAX_WAV_PEAK: f32 = 15.848_932;

pub(crate) fn decode(path: &Path, limit: usize) -> Result<StaticSoundData, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    check_file_size(file.metadata().map_err(|error| error.to_string())?.len())?;
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("wav");
    decode_source(extension, file, limit)
}

/// Validate an imported cue with the same codecs and frame limit as playback.
pub fn validate_audio_bytes(extension: &str, bytes: &[u8]) -> Result<(), String> {
    if !module_engine_core::AUDIO_CUE_EXTENSIONS
        .iter()
        .any(|supported| supported.eq_ignore_ascii_case(extension))
    {
        return Err("Unsupported audio file extension".into());
    }
    check_file_size(bytes.len() as u64)?;
    decode_source(extension, Cursor::new(bytes), MAX_FRAMES).map(|_| ())
}

fn check_file_size(bytes: u64) -> Result<(), String> {
    if bytes > MAX_FILE_BYTES {
        return Err("Audio file exceeds 128 MiB".into());
    }
    Ok(())
}

pub(crate) fn decode_source(
    extension: &str,
    source: impl Read + Seek,
    limit: usize,
) -> Result<StaticSoundData, String> {
    let (sample_rate, frames) = match extension.to_ascii_lowercase().as_str() {
        "ogg" => decode_vorbis(source, limit)?,
        "flac" => decode_flac(source, limit)?,
        "mp3" => decode_mp3(source, limit)?,
        _ => {
            // Legacy cue paths did not require a .wav extension.
            let wave = crate::wav::DecodedWav::read(source)?;
            let peak = wave
                .samples
                .iter()
                .map(|sample| sample.abs())
                .fold(0_f32, f32::max);
            if peak > MAX_WAV_PEAK {
                let dbfs = 20. * peak.log10();
                return Err(format!(
                    "WAV peak {dbfs:+.1} dBFS exceeds +24 dBFS. Lower the level of the source file."
                ));
            }
            let mut frames = Vec::new();
            append_frames(
                &mut frames,
                wave.samples.chunks_exact(2),
                2,
                limit,
                |sample| sample,
            )?;
            (wave.sample_rate, frames)
        }
    };
    if frames.is_empty() || !(8000..=192_000).contains(&sample_rate) {
        return Err("Audio clips need frames at an 8000..=192000 Hz sample rate".into());
    }
    Ok(StaticSoundData {
        sample_rate,
        frames: frames.into(),
        settings: Default::default(),
    })
}

fn decode_vorbis(source: impl Read + Seek, limit: usize) -> Result<(u32, Vec<Frame>), String> {
    let mut reader =
        lewton::inside_ogg::OggStreamReader::new(source).map_err(|error| error.to_string())?;
    let rate = reader.ident_hdr.audio_sample_rate;
    let channels = usize::from(reader.ident_hdr.audio_channels);
    if !matches!(channels, 1 | 2) {
        return Err("Audio cues must be mono or stereo".into());
    }
    let mut frames = Vec::new();
    while let Some(samples) = reader
        .read_dec_packet_itl()
        .map_err(|error| error.to_string())?
    {
        if rate != reader.ident_hdr.audio_sample_rate
            || channels != usize::from(reader.ident_hdr.audio_channels)
        {
            return Err("OGG/Vorbis audio layout changes within the clip".into());
        }
        append_frames(
            &mut frames,
            samples.chunks_exact(channels),
            channels,
            limit,
            |sample| f32::from(sample) / 32768.,
        )?;
    }
    Ok((rate, frames))
}

fn decode_flac(source: impl Read, limit: usize) -> Result<(u32, Vec<Frame>), String> {
    let mut reader = claxon::FlacReader::new(source).map_err(|error| error.to_string())?;
    let info = reader.streaminfo();
    if !matches!(info.channels, 1 | 2) {
        return Err("Audio cues must be mono or stereo".into());
    }
    if info.samples.is_some_and(|count| count > limit as u64) {
        return Err("Audio cues exceed the decoded frame budget".into());
    }
    let scale = 2_f32.powi(1 - info.bits_per_sample as i32);
    let mut frames = Vec::new();
    let mut samples = reader.samples();
    while let Some(left) = samples.next() {
        if frames.len() == limit {
            return Err("Audio cues exceed the decoded frame budget".into());
        }
        let left = left.map_err(|error| error.to_string())?;
        let right = if info.channels == 2 {
            samples
                .next()
                .ok_or("Incomplete stereo FLAC frame")?
                .map_err(|error| error.to_string())?
        } else {
            left
        };
        frames.push(Frame::new(left as f32 * scale, right as f32 * scale));
    }
    Ok((info.sample_rate, frames))
}

fn decode_mp3(source: impl Read, limit: usize) -> Result<(u32, Vec<Frame>), String> {
    // Nanomp3 needs contiguous input for frame synchronization.
    let mut input = Vec::new();
    source
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|error| error.to_string())?;
    check_file_size(input.len() as u64)?;
    let mut decoder = nanomp3::Decoder::new();
    let mut samples = [0.; nanomp3::MAX_SAMPLES_PER_FRAME];
    let mut layout = None;
    let mut frames = Vec::new();
    let mut remaining = input.as_slice();
    while !remaining.is_empty() {
        let (consumed, info) = decoder.decode(remaining, &mut samples);
        if consumed == 0 {
            break;
        }
        remaining = remaining
            .get(consumed..)
            .ok_or("Invalid MP3 frame length")?;
        if let Some(info) = info {
            let channels = usize::from(info.channels.num());
            let current = (info.sample_rate, channels);
            if *layout.get_or_insert(current) != current {
                return Err("MP3 audio layout changes within the clip".into());
            }
            let length = info.samples_produced * channels;
            let pcm = samples.get(..length).ok_or("Invalid MP3 sample count")?;
            append_frames(
                &mut frames,
                pcm.chunks_exact(channels),
                channels,
                limit,
                |sample| sample,
            )?;
        }
    }
    Ok((layout.ok_or("No MP3 frames")?.0, frames))
}

fn append_frames<'a, T: Copy + 'a>(
    frames: &mut Vec<Frame>,
    samples: impl ExactSizeIterator<Item = &'a [T]>,
    channels: usize,
    limit: usize,
    convert: impl Fn(T) -> f32,
) -> Result<(), String> {
    if samples.len() > limit - frames.len() {
        return Err("Audio cues exceed the decoded frame budget".into());
    }
    for samples in samples {
        let [left, right] = [convert(samples[0]), convert(samples[channels - 1])];
        if !left.is_finite() || !right.is_finite() {
            return Err("Non-finite decoded audio sample".into());
        }
        frames.push(Frame::new(left, right));
    }
    Ok(())
}
