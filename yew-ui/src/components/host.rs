use gloo_console::log;
use gloo_utils::window;
use js_sys::Array;
use js_sys::Boolean;
use js_sys::JsString;
use js_sys::Reflect;

use std::fmt::Debug;
use std::future::join;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use types::protos::media_packet::MediaPacket;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlVideoElement;
use web_sys::*;
use yew::prelude::*;

use crate::constants::AUDIO_CHANNELS;
use crate::constants::AUDIO_CODEC;
use crate::constants::AUDIO_SAMPLE_RATE;
use crate::constants::VIDEO_CODEC;
use crate::constants::VIDEO_HEIGHT;
use crate::constants::VIDEO_WIDTH;
use crate::model::transform_audio_chunk;
use crate::model::transform_screen_chunk;
use crate::model::transform_video_chunk;

pub enum Msg {
    Start,
    EnableScreenShare,
    EnableMicrophone(bool),
    EnableVideo(bool),
}

pub struct Host {
    pub destroy: Arc<AtomicBool>,
    pub share_screen: Arc<AtomicBool>,
    pub mute_microphone: Arc<AtomicBool>,
    pub share_video: Arc<AtomicBool>,
}

#[derive(Properties, Debug, PartialEq)]
pub struct MeetingProps {
    #[prop_or_default]
    pub id: String,

    #[prop_or_default]
    pub on_frame: Callback<MediaPacket>,

    #[prop_or_default]
    pub on_audio: Callback<MediaPacket>,

    #[prop_or_default]
    pub email: String,

    #[prop_or_default]
    pub share_screen: bool,

    #[prop_or_default]
    pub mute_microphone: bool,

    #[prop_or_default]
    pub share_video: bool,
}

impl Component for Host {
    type Message = Msg;
    type Properties = MeetingProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            destroy: Arc::new(AtomicBool::new(false)),
            share_screen: Arc::new(AtomicBool::new(false)),
            mute_microphone: Arc::new(AtomicBool::new(false)),
            share_video: Arc::new(AtomicBool::new(false)),
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        // Determine if we should start/stop screen share.
        if ctx.props().share_screen != self.share_screen.load(Ordering::Acquire) {
            self.share_screen
                .store(ctx.props().share_screen, Ordering::Release);
            if ctx.props().share_screen {
                ctx.link().send_message(Msg::EnableScreenShare);
            }
        }
        // Determine if we should start/stop microphone.
        if ctx.props().mute_microphone != self.mute_microphone.load(Ordering::Acquire) {
            self.mute_microphone
                .store(ctx.props().mute_microphone, Ordering::Release);
            ctx.link()
                .send_message(Msg::EnableMicrophone(!ctx.props().mute_microphone));
        }
        // Determine if we should start/stop video.
        if ctx.props().share_video != self.share_video.load(Ordering::Acquire) {
            self.share_video
                .store(ctx.props().share_video, Ordering::Release);
            ctx.link()
                .send_message(Msg::EnableVideo(ctx.props().share_video));
        }

        if first_render {
            ctx.link().send_message(Msg::Start);
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::EnableScreenShare => {
                let share_screen = self.share_screen.clone();
                let on_frame = Box::new(ctx.props().on_frame.clone());
                let email = Box::new(ctx.props().email.clone());
                let destroy = self.destroy.clone();
                let screen_output_handler = {
                    let email = email.clone();
                    let on_frame = on_frame.clone();
                    let mut buffer: [u8; 100000] = [0; 100000];
                    Box::new(move |chunk: JsValue| {
                        let chunk = web_sys::EncodedVideoChunk::from(chunk);
                        let media_packet: MediaPacket =
                            transform_screen_chunk(chunk, &mut buffer, email.clone());
                        on_frame.emit(media_packet);
                    })
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let navigator = window().navigator();
                    let media_devices = navigator.media_devices().unwrap();
                    let screen_to_share: MediaStream =
                        JsFuture::from(media_devices.get_display_media().unwrap())
                            .await
                            .unwrap()
                            .unchecked_into::<MediaStream>();

                    let screen_track = Box::new(
                        screen_to_share
                            .get_video_tracks()
                            .find(&mut |_: JsValue, _: u32, _: Array| true)
                            .unchecked_into::<VideoTrack>(),
                    );

                    let screen_error_handler = Closure::wrap(Box::new(move |e: JsValue| {
                        log!("error_handler error", e);
                    })
                        as Box<dyn FnMut(JsValue)>);

                    let screen_output_handler =
                        Closure::wrap(screen_output_handler as Box<dyn FnMut(JsValue)>);

                    let screen_encoder_init = VideoEncoderInit::new(
                        screen_error_handler.as_ref().unchecked_ref(),
                        screen_output_handler.as_ref().unchecked_ref(),
                    );

                    let screen_encoder = Box::new(VideoEncoder::new(&screen_encoder_init).unwrap());
                    let mut screen_encoder_config = VideoEncoderConfig::new(
                        &VIDEO_CODEC,
                        VIDEO_HEIGHT as u32,
                        VIDEO_WIDTH as u32,
                    );
                    screen_encoder_config.bitrate(100_000f64);
                    screen_encoder_config.latency_mode(LatencyMode::Realtime);
                    screen_encoder.configure(&screen_encoder_config);

                    let screen_processor =
                        MediaStreamTrackProcessor::new(&MediaStreamTrackProcessorInit::new(
                            &screen_track.unchecked_into::<MediaStreamTrack>(),
                        ))
                        .unwrap();

                    let screen_reader = screen_processor
                        .readable()
                        .get_reader()
                        .unchecked_into::<ReadableStreamDefaultReader>();

                    let mut screen_frame_counter = 0;

                    let poll_screen = async {
                        loop {
                            if destroy.load(Ordering::Acquire) {
                                return;
                            }
                            if !share_screen.load(Ordering::Acquire) {
                                return;
                            }
                            match JsFuture::from(screen_reader.read()).await {
                                Ok(js_frame) => {
                                    log!("");
                                    let video_frame =
                                        Reflect::get(&js_frame, &JsString::from("value"))
                                            .unwrap()
                                            .unchecked_into::<VideoFrame>();
                                    let mut opts = VideoEncoderEncodeOptions::new();
                                    screen_frame_counter = (screen_frame_counter + 1) % 50;
                                    opts.key_frame(screen_frame_counter == 0);
                                    screen_encoder.encode_with_options(&video_frame, &opts);
                                    video_frame.close();
                                }
                                Err(e) => {
                                    log!("error", e);
                                }
                            }
                        }
                    };
                    poll_screen.await;
                });
                true
            }
            Msg::Start => {
                // 1. Query the first device with a camera and a mic attached.
                // 2. setup WebCodecs, in particular
                // 3. send encoded video frames and raw audio to the server.
                let on_frame = Box::new(ctx.props().on_frame.clone());
                let on_audio = Box::new(ctx.props().on_audio.clone());
                let email = Box::new(ctx.props().email.clone());

                let video_output_handler = {
                    let email = email.clone();
                    let on_frame = on_frame.clone();
                    let mut buffer: [u8; 100000] = [0; 100000];
                    Box::new(move |chunk: JsValue| {
                        let chunk = web_sys::EncodedVideoChunk::from(chunk);
                        let media_packet: MediaPacket =
                            transform_video_chunk(chunk, &mut buffer, email.clone());
                        on_frame.emit(media_packet);
                    })
                };

                let audio_output_handler = {
                    let email = email.clone();
                    let on_audio = on_audio.clone();
                    let mut buffer: [u8; 100000] = [0; 100000];
                    log!("Handling audio");
                    Box::new(move |chunk: JsValue| {
                        let chunk = web_sys::EncodedAudioChunk::from(chunk);
                        let media_packet: MediaPacket =
                            transform_audio_chunk(&chunk, &mut buffer, &email);
                        on_audio.emit(media_packet);
                    })
                };
                let destroy = self.destroy.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let navigator = window().navigator();

                    let media_devices = navigator.media_devices().unwrap();

                    let video_element = window()
                        .document()
                        .unwrap()
                        .get_element_by_id("webcam")
                        .unwrap()
                        .unchecked_into::<HtmlVideoElement>();

                    // TODO: Add dropdown so that user can select the device that they want to use.
                    let mut constraints = MediaStreamConstraints::new();
                    constraints.video(&Boolean::from(true));
                    constraints.audio(&Boolean::from(true));
                    let devices_query = media_devices
                        .get_user_media_with_constraints(&constraints)
                        .unwrap();
                    let device = JsFuture::from(devices_query)
                        .await
                        .unwrap()
                        .unchecked_into::<MediaStream>();
                    video_element.set_src_object(Some(&device));
                    video_element.set_muted(true);
                });
                true
            }

            Msg::EnableMicrophone(_) => {
                let on_audio = Box::new(ctx.props().on_audio.clone());
                let email = Box::new(ctx.props().email.clone());

                let audio_output_handler = {
                    let email = email.clone();
                    let on_audio = on_audio.clone();
                    let mut buffer: [u8; 100000] = [0; 100000];
                    log!("Handling audio");
                    Box::new(move |chunk: JsValue| {
                        let chunk = web_sys::EncodedAudioChunk::from(chunk);
                        let media_packet: MediaPacket =
                            transform_audio_chunk(&chunk, &mut buffer, &email);
                        on_audio.emit(media_packet);
                    })
                };
                let destroy = self.destroy.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let navigator = window().navigator();
                    let media_devices = navigator.media_devices().unwrap();
                    // TODO: Add dropdown so that user can select the device that they want to use.
                    let mut constraints = MediaStreamConstraints::new();
                    constraints.video(&Boolean::from(true));
                    constraints.audio(&Boolean::from(true));
                    let devices_query = media_devices
                        .get_user_media_with_constraints(&constraints)
                        .unwrap();
                    let device = JsFuture::from(devices_query)
                        .await
                        .unwrap()
                        .unchecked_into::<MediaStream>();

                    // Setup audio encoder.

                    let audio_error_handler = Closure::wrap(Box::new(move |e: JsValue| {
                        log!("error_handler error", e);
                    })
                        as Box<dyn FnMut(JsValue)>);

                    let audio_output_handler =
                        Closure::wrap(audio_output_handler as Box<dyn FnMut(JsValue)>);

                    let audio_encoder_init = AudioEncoderInit::new(
                        audio_error_handler.as_ref().unchecked_ref(),
                        audio_output_handler.as_ref().unchecked_ref(),
                    );
                    let audio_encoder = Box::new(AudioEncoder::new(&audio_encoder_init).unwrap());
                    let audio_track = Box::new(
                        device
                            .get_audio_tracks()
                            .find(&mut |_: JsValue, _: u32, _: Array| true)
                            .unchecked_into::<AudioTrack>(),
                    );
                    let mut audio_encoder_config = AudioEncoderConfig::new(&AUDIO_CODEC);
                    audio_encoder_config.bitrate(100_000f64);
                    audio_encoder_config.sample_rate(AUDIO_SAMPLE_RATE as u32);
                    audio_encoder_config.number_of_channels(AUDIO_CHANNELS as u32);
                    audio_encoder.configure(&audio_encoder_config);

                    let audio_processor =
                        MediaStreamTrackProcessor::new(&MediaStreamTrackProcessorInit::new(
                            &audio_track.unchecked_into::<MediaStreamTrack>(),
                        ))
                        .unwrap();
                    let audio_reader = audio_processor
                        .readable()
                        .get_reader()
                        .unchecked_into::<ReadableStreamDefaultReader>();

                    // Start encoding video and audio.
                    let mut video_frame_counter = 0;

                    let poll_audio = async {
                        loop {
                            if destroy.load(Ordering::Acquire) {
                                return;
                            }
                            match JsFuture::from(audio_reader.read()).await {
                                Ok(js_frame) => {
                                    let audio_frame =
                                        Reflect::get(&js_frame, &JsString::from("value"))
                                            .unwrap()
                                            .unchecked_into::<AudioData>();
                                    audio_encoder.encode(&audio_frame);
                                    audio_frame.close();
                                }
                                Err(e) => {
                                    log!("error", e);
                                }
                            }
                        }
                    };

                    join!(poll_audio).await;
                    log!("Killing streamer");
                });
                true
            }
            Msg::EnableVideo(_) => {
                // 1. Query the first device with a camera and a mic attached.
                // 2. setup WebCodecs, in particular
                // 3. send encoded video frames and raw audio to the server.
                let on_frame = Box::new(ctx.props().on_frame.clone());
                let email = Box::new(ctx.props().email.clone());

                let video_output_handler = {
                    let email = email.clone();
                    let on_frame = on_frame.clone();
                    let mut buffer: [u8; 100000] = [0; 100000];
                    Box::new(move |chunk: JsValue| {
                        let chunk = web_sys::EncodedVideoChunk::from(chunk);
                        let media_packet: MediaPacket =
                            transform_video_chunk(chunk, &mut buffer, email.clone());
                        on_frame.emit(media_packet);
                    })
                };

                let destroy = self.destroy.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let navigator = window().navigator();

                    let media_devices = navigator.media_devices().unwrap();
                    // TODO: Add dropdown so that user can select the device that they want to use.
                    let mut constraints = MediaStreamConstraints::new();
                    constraints.video(&Boolean::from(true));
                    constraints.audio(&Boolean::from(true));
                    let devices_query = media_devices
                        .get_user_media_with_constraints(&constraints)
                        .unwrap();
                    let device = JsFuture::from(devices_query)
                        .await
                        .unwrap()
                        .unchecked_into::<MediaStream>();

                    let video_track = Box::new(
                        device
                            .get_video_tracks()
                            .find(&mut |_: JsValue, _: u32, _: Array| true)
                            .unchecked_into::<VideoTrack>(),
                    );

                    // Setup video encoder

                    let video_error_handler = Closure::wrap(Box::new(move |e: JsValue| {
                        log!("error_handler error", e);
                    })
                        as Box<dyn FnMut(JsValue)>);

                    let video_output_handler =
                        Closure::wrap(video_output_handler as Box<dyn FnMut(JsValue)>);

                    let video_encoder_init = VideoEncoderInit::new(
                        video_error_handler.as_ref().unchecked_ref(),
                        video_output_handler.as_ref().unchecked_ref(),
                    );

                    let video_encoder = Box::new(VideoEncoder::new(&video_encoder_init).unwrap());

                    let video_settings = &mut video_track
                        .clone()
                        .unchecked_into::<MediaStreamTrack>()
                        .get_settings();
                    video_settings.width(VIDEO_WIDTH);
                    video_settings.height(VIDEO_HEIGHT);

                    let mut video_encoder_config = VideoEncoderConfig::new(
                        &VIDEO_CODEC,
                        VIDEO_HEIGHT as u32,
                        VIDEO_WIDTH as u32,
                    );

                    video_encoder_config.bitrate(100_000f64);
                    video_encoder_config.latency_mode(LatencyMode::Realtime);
                    video_encoder.configure(&video_encoder_config);

                    let video_processor =
                        MediaStreamTrackProcessor::new(&MediaStreamTrackProcessorInit::new(
                            &video_track.unchecked_into::<MediaStreamTrack>(),
                        ))
                        .unwrap();
                    let video_reader = video_processor
                        .readable()
                        .get_reader()
                        .unchecked_into::<ReadableStreamDefaultReader>();

                    // Setup audio encoder.

                    // Start encoding video and audio.
                    let mut video_frame_counter = 0;
                    let poll_video = async {
                        loop {
                            if destroy.load(Ordering::Acquire) {
                                return;
                            }
                            match JsFuture::from(video_reader.read()).await {
                                Ok(js_frame) => {
                                    let video_frame =
                                        Reflect::get(&js_frame, &JsString::from("value"))
                                            .unwrap()
                                            .unchecked_into::<VideoFrame>();
                                    let mut opts = VideoEncoderEncodeOptions::new();
                                    video_frame_counter = (video_frame_counter + 1) % 50;
                                    opts.key_frame(video_frame_counter == 0);
                                    video_encoder.encode_with_options(&video_frame, &opts);
                                    video_frame.close();
                                }
                                Err(e) => {
                                    log!("error", e);
                                }
                            }
                        }
                    };
                    join!(poll_video).await;
                    log!("Killing streamer");
                });
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <video class="self-camera" autoplay=true id="webcam"></video>
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        log!("destroying");
        self.destroy.store(true, Ordering::Release);
    }
}
