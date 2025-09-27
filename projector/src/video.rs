
use anyhow::{anyhow, Error, Result};
use fraction::Fraction;
use gstreamer::{self as gst, SeekFlags};
use gstreamer::{prelude::*, ClockTime};
use gstreamer_pbutils::prelude::DiscovererStreamInfoExt;
use gstreamer_pbutils::Discoverer;
use log::warn;
use raylib::color::Color;
use raylib::math::Vector2;
use raylib::prelude::RaylibDraw;
use raylib::texture::{Image, RaylibTexture2D, Texture2D};
use raylib::{RaylibHandle, RaylibThread};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, fmt};
use shared::path::path_to_file_uri;

#[derive(Debug)]
pub struct AudioMetadata {
    media_type: String,
    bitrate: u32,
    max_bitrate: u32,
    channel_mask: u64,
    channels: u32,
    depth: u32,
    sample_rate: u32,
}

pub struct RaylibVideo {
    pub duration: Duration,
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub max_bitrate: u32,
    pub depth: u32,
    pub framerate: Fraction,
    pub is_interlaced: bool,
    pub par: Fraction,
    pub media_type: String,

    pub audio_meta: Option<AudioMetadata>,

    pub timestamp_ms: Arc<AtomicU64>,

    paused: bool,
    rate: f64,

    pipeline: gst::Pipeline,

    video_frame: Arc<Mutex<Vec<u8>>>,
    video_frame_is_dirty: Arc<AtomicBool>,

    // Raylib specific
    pub frame_texture: Texture2D,
}

impl RaylibVideo {
    pub(crate) fn new(
        path: &str,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) -> anyhow::Result<RaylibVideo> {
        gst::init()?;

        if !Path::new(path).exists() {
            return Err(anyhow!("Video file not found: {}", path));
        }

        let discoverer = Discoverer::new(ClockTime::from_seconds(5))
            .map_err(|e| anyhow!("Failed to create GStreamer discoverer: {}", e))?;

        let path_canonical = Path::new(path)
            .canonicalize()
            .map_err(|e| anyhow!("Failed to get canonical path for '{}': {}", path, e))?;

        let path_canonical_str = path_canonical
            .to_str()
            .ok_or_else(|| anyhow!("Path contains invalid Unicode: {:?}", path_canonical))?;

        let uri = path_to_file_uri(&path_canonical)
            .map_err(|e| anyhow!("Failed to get uri: {}", e))?;
        let info = discoverer
            .discover_uri(&uri)
            .map_err(|e| anyhow!("Failed to discover media information for '{}': {}", path, e))?;

        let video_duration = info
            .duration()
            .ok_or_else(|| anyhow!("Cannot determine media duration for '{}'", path))?;
        let video_duration_msec = video_duration.mseconds();
        let video_duration = Duration::from_millis(video_duration_msec);

        let video_streams = info.video_streams();
        if video_streams.is_empty() {
            return Err(anyhow!("No video streams found in '{}'", path));
        }

        if video_streams.len() > 1 {
            warn!(
                "Video '{}' has {} video streams. Only the first one will be used.",
                path,
                video_streams.len()
            );
        }

        let video = &video_streams[0];
        let framerate_frac = video.framerate();

        if framerate_frac < 0.into() {
            return Err(anyhow!("Invalid negative framerate in '{}'", path));
        }

        let video_width = video.width();
        let video_height = video.height();

        if video_width == 0 || video_height == 0 {
            return Err(anyhow!(
                "Invalid video dimensions ({}x{}) in '{}'",
                video_width,
                video_height,
                path
            ));
        }

        let video_bitrate = video.bitrate();
        let video_max_bitrate = video.max_bitrate();
        let video_depth = video.depth();
        let video_is_interlaced = video.is_interlaced();
        let video_framerate =
            Fraction::new(framerate_frac.numer() as u32, framerate_frac.denom() as u32);
        let video_par = Fraction::new(video.par().numer() as u32, video.par().denom() as u32);

        let mut video_media_type = "video/*".to_string();

        if let Some(caps) = video.caps() {
            if caps.iter().len() > 1 {
                warn!("Video stream has multiple caps. Only the first one will be used.");
            }

            for c in caps.iter() {
                video_media_type = c.name().to_string();
                break;
            }
        }

        let mut audio_info: Option<AudioMetadata> = None;
        let audio_streams = info.audio_streams();

        if audio_streams.len() > 1 {
            warn!(
                "Video '{}' has {} audio streams. Only the first one will be used.",
                path,
                audio_streams.len()
            );
        }

        if let Some(audio) = audio_streams.get(0) {
            let mut info = AudioMetadata {
                media_type: "audio/*".to_string(),
                bitrate: audio.bitrate(),
                max_bitrate: audio.max_bitrate(),
                channel_mask: audio.channel_mask(),
                channels: audio.channels(),
                depth: audio.depth(),
                sample_rate: audio.sample_rate(),
            };

            if let Some(caps) = audio.caps() {
                if caps.iter().len() > 1 {
                    warn!("Audio stream has multiple caps. Only the first one will be used.");
                }

                for c in caps.iter() {
                    info.media_type = c.name().to_string();
                    break;
                }
            }

            audio_info = Some(info)
        }

        let pipeline_str = format!(
            "filesrc location=\"{}\" ! decodebin name=decode ! queue ! videoconvert ! video/x-raw,format=RGB,width={},height={},colorimetry=sRGB ! appsink name=appsink sync=true decode. ! queue ! audioconvert !volume volume=0.1 ! audioresample ! autoaudiosink",
            path_canonical_str, video_width, video_height
        );

        let pipeline = gstreamer::parse::launch(&pipeline_str)
            .map_err(|e| anyhow!("Failed to create pipeline: {}", e))?;
        let pipeline = pipeline
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow!("Failed to downcast pipeline"))?;

        // Get the appsink element
        let appsink = pipeline
            .by_name("appsink")
            .ok_or_else(|| anyhow!("Failed to get appsink element"))?
            .downcast::<gstreamer_app::AppSink>()
            .map_err(|_| anyhow!("Failed to downcast appsink"))?;

        let weak_pipe = pipeline.downgrade();
        let ts_ref = Arc::new(AtomicU64::new(0));
        let ts_ref_clone = ts_ref.clone();
        let frame_ref = Arc::new(Mutex::new(vec![
            0;
            video_width as usize
                * video_height as usize
                * 3
        ]));
        let frame_ref_clone = frame_ref.clone();
        let dirtiness_ref = Arc::new(AtomicBool::new(false));
        let dirtiness_ref_clone = dirtiness_ref.clone();

        // Set up appsink callbacks
        appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;

                    if let Some(pipeline) = weak_pipe.upgrade() {
                        if let Some(v) = pipeline.query_position::<gst::ClockTime>() {
                            ts_ref_clone.store(v.mseconds(), Ordering::Relaxed);
                        }
                    }

                    // Lock the texture buffer for updating
                    if let Ok(mut frame_data) = frame_ref_clone.lock() {
                        frame_data.copy_from_slice(&map);
                        dirtiness_ref_clone.store(true, Ordering::Relaxed);
                    } else {
                        eprintln!("Failed to lock texture buffer");
                    }

                    return Ok(gst::FlowSuccess::Ok);
                })
                .build(),
        );

        let video = RaylibVideo {
            pipeline,
            paused: true,
            rate: 1.0,
            timestamp_ms: ts_ref,
            bitrate: video_bitrate,
            depth: video_depth,
            width: video_width,
            height: video_height,
            duration: video_duration,
            framerate: video_framerate,
            is_interlaced: video_is_interlaced,
            max_bitrate: video_max_bitrate,
            media_type: video_media_type,
            par: video_par,
            audio_meta: audio_info,

            video_frame: frame_ref,
            video_frame_is_dirty: dirtiness_ref,

            frame_texture: init_empty_texture(video_width, video_height, rl, thread)?,
        };

        return Ok(video);
    }

    pub(crate) fn play(&mut self) {
        self.paused = false;
        if let Err(err) = self.pipeline.set_state(gst::State::Playing) {
            warn!("Error while changing pipeline state to Playing: {}", err)
        }
    }

    fn pause(&mut self) {
        self.paused = true;
        if let Err(err) = self.pipeline.set_state(gst::State::Paused) {
            warn!("Error while changing pipeline state to Paused: {}", err)
        }
    }

    fn seek(&self, time_ms: i64) {
        let t = time_ms.clamp(0, self.duration.as_millis() as i64) as u64;
        let target_ts = gst::ClockTime::from_mseconds(t);
        self.timestamp_ms.store(t, Ordering::Relaxed);

        if let Err(err) = self.pipeline.seek(
            self.rate,
            SeekFlags::FLUSH | SeekFlags::KEY_UNIT | SeekFlags::TRICKMODE,
            gstreamer::SeekType::Set,
            target_ts,
            gstreamer::SeekType::End,
            ClockTime::ZERO,
        ) {
            warn!("Failed to set video rate: {}", err)
        }
        return;
    }

    fn seek_relative(&self, time_ms: i64) {
        self.seek(self.get_timestamp() as i64 + time_ms);
    }

    pub(crate) fn is_finished(&self) -> bool {
        if let Some(bus) = self.pipeline.bus() {
            while let Some(msg) = bus.pop() {
                if msg.type_() == gst::MessageType::Eos {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn wait_until_finished(&self) {
        let bus = match self.pipeline.bus() {
            Some(b) => b,
            None => return,
        };
        for _ in bus.iter_timed(ClockTime::NONE) {}
    }

    fn get_timestamp(&self) -> u64 {
        return self.timestamp_ms.load(Ordering::Relaxed);
    }

    pub(crate) fn update(&mut self) {
        let dirty = self.video_frame_is_dirty.load(Ordering::Relaxed);
        if !dirty {
            return;
        }

        self.video_frame_is_dirty.store(false, Ordering::Relaxed);
        if let Ok(frame_data) = self.video_frame.lock() {
            let result = self.frame_texture.update_texture(&frame_data);
            if let Err(err) = result {
                warn!("Failed to update video texture data: {}", err)
            }
        } else {
            warn!("Failed to obtain video frame lock")
        }
    }

    /// Setting rate too high might not reflect the actual playback rate
    fn set_rate(&mut self, rate: f64) {
        self.rate = rate.max(0.01);
        let curr = self.get_timestamp();
        if let Err(err) = self.pipeline.seek(
            self.rate,
            SeekFlags::FLUSH | SeekFlags::KEY_UNIT | SeekFlags::TRICKMODE,
            gstreamer::SeekType::Set,
            gst::ClockTime::from_mseconds(curr),
            gstreamer::SeekType::End,
            ClockTime::ZERO,
        ) {
            warn!("Failed to set video rate: {}", err)
        }
    }

    fn get_rate(&self) -> f64 {
        return self.rate;
    }
}

impl Drop for RaylibVideo {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn init_empty_texture(
    width: u32,
    height: u32,
    rl: &mut RaylibHandle,
    t: &RaylibThread
) -> Result<Texture2D, Error> {
    let img = Image::gen_image_color(
        width as i32,
        height as i32,
        raylib::ffi::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0
        },
    );

    let mut texture = rl.load_texture_from_image(&t, &img)?;
    texture.format = 4; // PIXELFORMAT_UNCOMPRESSED_R8G8B8
    return Ok(texture);
}
