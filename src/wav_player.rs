use hound::{self,WavSpec,WavReader};
use std::{i16, path, fs::File};
use cpal::{ Host, Device,
            traits::{DeviceTrait, HostTrait, StreamTrait}};
use anyhow::{Result,Context};
use dasp_signal::{self, Signal};
use dasp_slice::ToFrameSliceMut;

pub fn play_wav_file<P: AsRef<path::Path>>(filename: P) -> Result<()>{
    
    let reader: WavReader<std::io::BufReader<File>> = hound::WavReader::open(filename)?;
    let spec: WavSpec = reader.spec();
    println!("Houssem: {}",spec.sample_rate);

    // Read the interleaved samples and convert them to a signal.
    let samples = reader.into_samples::<i16>().filter_map(Result::ok);
    let mut frames = dasp_signal::from_interleaved_samples_iter(samples).until_exhausted();

    // Initialise CPAL.
    let host: Host = cpal::default_host();
    let device: Device = host.default_output_device().context("failed to find a default output device")?;

    // Create a stream config to match the wave format.
    let config = cpal::StreamConfig {
        channels: spec.channels,
        sample_rate: cpal::SampleRate(spec.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    //debug
    println!("Device: {:?}", device.name());
    println!("Config: {:?}", config);

    // A channel for indicating when playback has completed.
    let (complete_tx, complete_rx) = std::sync::mpsc::sync_channel(1);

    // Create and run the CPAL stream.
    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);
    let data_fn = move |data: &mut [i16], _info: &cpal::OutputCallbackInfo| {
        let buffer: &mut [[i16; 2]] = data.to_frame_slice_mut().unwrap();
        for out_frame in buffer {
            match frames.next() {
                Some(frame) => *out_frame = frame,
                None => {
                    complete_tx.try_send(()).ok();
                    *out_frame = dasp::Frame::EQUILIBRIUM;
                }
            }
        }
    };


    let stream = device.build_output_stream(&config, data_fn, err_fn, None)?;
    stream.play()?;

    // Block until playback completes.
    complete_rx.recv()?;
    stream.pause().ok();

    Ok(())
}