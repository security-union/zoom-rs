use protobuf::Message;
use std::fmt;
use types::protos::rust::media_packet::MediaPacket;
use web_sys::*;
use yew_websocket::websocket::{Binary, Text};
pub struct MediaPacketWrapper(pub MediaPacket);

impl From<Text> for MediaPacketWrapper {
    fn from(_: Text) -> Self {
        MediaPacketWrapper(MediaPacket::default())
    }
}

impl From<Binary> for MediaPacketWrapper {
    fn from(bin: Binary) -> Self {
        let media_packet: MediaPacket = bin
            .map(|data| MediaPacket::parse_from_bytes(&data.into_boxed_slice()).unwrap())
            .unwrap_or(MediaPacket::default());
        MediaPacketWrapper(media_packet)
    }
}

pub struct EncodedVideoChunkTypeWrapper(pub EncodedVideoChunkType);

impl From<String> for EncodedVideoChunkTypeWrapper {
    fn from(s: String) -> Self {
        match s.as_str() {
            "key" => EncodedVideoChunkTypeWrapper(EncodedVideoChunkType::Key),
            _ => EncodedVideoChunkTypeWrapper(EncodedVideoChunkType::Delta),
        }
    }
}

impl fmt::Display for EncodedVideoChunkTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            EncodedVideoChunkType::Delta => write!(f, "delta"),
            EncodedVideoChunkType::Key => write!(f, "key"),
            _ => todo!(),
        }
    }
}

pub struct EncodedAudioChunkTypeWrapper(pub EncodedAudioChunkType);

impl From<String> for EncodedAudioChunkTypeWrapper {
    fn from(s: String) -> Self {
        match s.as_str() {
            "key" => EncodedAudioChunkTypeWrapper(EncodedAudioChunkType::Key),
            _ => EncodedAudioChunkTypeWrapper(EncodedAudioChunkType::Delta),
        }
    }
}

impl fmt::Display for EncodedAudioChunkTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            EncodedAudioChunkType::Delta => write!(f, "delta"),
            EncodedAudioChunkType::Key => write!(f, "key"),
            _ => todo!(),
        }
    }
}

pub struct AudioSampleFormatWrapper(pub AudioSampleFormat);

impl From<String> for AudioSampleFormatWrapper {
    fn from(s: String) -> Self {
        match s.as_str() {
            "u8" => AudioSampleFormatWrapper(AudioSampleFormat::U8),
            "s16" => AudioSampleFormatWrapper(AudioSampleFormat::S16),
            "s32" => AudioSampleFormatWrapper(AudioSampleFormat::S32),
            "f32" => AudioSampleFormatWrapper(AudioSampleFormat::F32),
            "u8-planar" => AudioSampleFormatWrapper(AudioSampleFormat::U8Planar),
            "s16-planar" => AudioSampleFormatWrapper(AudioSampleFormat::S16Planar),
            "s32-planar" => AudioSampleFormatWrapper(AudioSampleFormat::S32Planar),
            "f32-planar" => AudioSampleFormatWrapper(AudioSampleFormat::F32Planar),
            _ => todo!(),
        }
    }
}

impl fmt::Display for AudioSampleFormatWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            AudioSampleFormat::U8 => write!(f, "u8"),
            AudioSampleFormat::S16 => write!(f, "s16"),
            AudioSampleFormat::S32 => write!(f, "s32"),
            AudioSampleFormat::F32 => write!(f, "f32"),
            AudioSampleFormat::U8Planar => write!(f, "u8-planar"),
            AudioSampleFormat::S16Planar => write!(f, "s16-planar"),
            AudioSampleFormat::S32Planar => write!(f, "s32-planar"),
            AudioSampleFormat::F32Planar => write!(f, "f32-planar"),
            _ => todo!(),
        }
    }
}
