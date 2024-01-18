use std::io::prelude::*;
use std::net::TcpStream;
use std::io;
use std::path;
use std::fs;
use hound;
use serde_json::to_writer;
use std::io::BufReader;
use serde::{Deserialize, Serialize};
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp_signal::{self, Signal};
use dasp_slice::ToFrameSliceMut;

const BYTE_BUFFER_SIZE: usize = (u16::MAX as usize) + (1 as usize);
const I8_SIZE : usize = 1;
const I16_SIZE : usize = 2;
const I32_SIZE : usize = 4;
const F32_SIZE : usize = 4;

#[derive(Serialize, Deserialize)]
enum Command {
    PerformRecording,
    SendFilesList,
    SendFileSpecs,
    SendFile,
}

#[derive(Serialize, Deserialize)]
pub enum SampleFormat {
    Float,
    Int,
}

#[derive(Serialize, Deserialize)]
struct WavFileSpecs {
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    sample_format: SampleFormat,
    samples_amount: u32,
}

fn wav_spec_from_wav_file_specs(wav_file_specs: WavFileSpecs) -> std::io::Result<hound::WavSpec> {

    let sample_format: hound::SampleFormat = match wav_file_specs.sample_format {
        SampleFormat::Float => hound::SampleFormat::Float,
        SampleFormat::Int => hound::SampleFormat::Int,
        _ => {panic!("Unvalid WavFileSpecs sample format!");},
    };

    Ok(hound::WavSpec {
        channels: wav_file_specs.channels,
        sample_rate: wav_file_specs.sample_rate,
        bits_per_sample: wav_file_specs.bits_per_sample,
        sample_format: sample_format,
    })
}

fn wav_file_specs_from_wav_spec(wav_spec: hound::WavSpec, samples_amount: u32) -> std::io::Result<WavFileSpecs> {

    let sample_format: SampleFormat = match wav_spec.sample_format {
        hound::SampleFormat::Float => SampleFormat::Float,
        hound::SampleFormat::Int => SampleFormat::Int ,
        _ => {panic!("Unvalid WavFileSpecs sample format!");},
    };

    Ok(WavFileSpecs  {
        channels: wav_spec.channels,
        sample_rate: wav_spec.sample_rate,
        bits_per_sample: wav_spec.bits_per_sample,
        sample_format,
        samples_amount,
    })
}

#[derive(Serialize, Deserialize)]
struct ClientRequest {
    command: Command,
    filename: String,
}

#[derive(Serialize, Deserialize)]
struct FilesList {
    files: Vec<String>,
}

fn bytes_to_audio(audio_bytes_buffer:[u8;(i16::MAX as usize *2) as usize]) -> [i16;i16::MAX as usize] {
    let mut audio_buffer : [i16;i16::MAX as usize] = [0;i16::MAX as usize];

    for i in 0..audio_buffer.len() {
        audio_buffer[i] = ((audio_bytes_buffer[i*2] as i16) << 8) | (audio_bytes_buffer[i*2 + 1] as i16);
    }

    return audio_buffer;
}

fn tcp_receive_chunk(mut stream: &TcpStream) -> std::io::Result<[i16;i16::MAX as usize]>{
    let mut audio_bytes_buffer: [u8;(i16::MAX as usize *2) as usize] = [0;(i16::MAX as usize *2) as usize];

    stream.read_exact(&mut audio_bytes_buffer)?;

    Ok(bytes_to_audio(audio_bytes_buffer))
}

fn tcp_receive_bytes_chunk(mut stream: &TcpStream) -> std::io::Result<[u8;BYTE_BUFFER_SIZE]>{
    let mut audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE] = [0;BYTE_BUFFER_SIZE];

    stream.read_exact(&mut audio_bytes_buffer)?;

    Ok(audio_bytes_buffer)
}

fn bytes_buffer_to_i8(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [i8;BYTE_BUFFER_SIZE]{
    let mut audio_buffer : [i8;BYTE_BUFFER_SIZE/I8_SIZE] = [0;BYTE_BUFFER_SIZE/I8_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index = audio_buffer_index * I8_SIZE;
        audio_buffer[audio_buffer_index]  = audio_bytes_buffer[index] as i8;
    }

    return audio_buffer;
}

fn bytes_buffer_to_i16(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [i16;BYTE_BUFFER_SIZE/I16_SIZE]{
    let mut audio_buffer : [i16;BYTE_BUFFER_SIZE/I16_SIZE] = [0;BYTE_BUFFER_SIZE/I16_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index = audio_buffer_index * I16_SIZE;
        audio_buffer[audio_buffer_index] = ((audio_bytes_buffer[index] as i16) << 8)
                                            | (audio_bytes_buffer[index + 1] as i16);
    }

    return audio_buffer;
}

fn bytes_buffer_to_i32(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [i32;BYTE_BUFFER_SIZE/I32_SIZE]{
    let mut audio_buffer : [i32;BYTE_BUFFER_SIZE/I32_SIZE] = [0;BYTE_BUFFER_SIZE/I32_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index = audio_buffer_index * I32_SIZE;
        audio_buffer[audio_buffer_index] = ((audio_bytes_buffer[index] as i32) << 24)
                                            | ((audio_bytes_buffer[index + 1] as i32) << 16)
                                            | ((audio_bytes_buffer[index + 2] as i32) << 8)
                                            | (audio_bytes_buffer[index + 3] as i32);
    }

    return audio_buffer;
}

fn bytes_buffer_to_f32(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [f32;BYTE_BUFFER_SIZE/F32_SIZE]{
    let mut audio_buffer : [f32;BYTE_BUFFER_SIZE/F32_SIZE] = [0.0;BYTE_BUFFER_SIZE/F32_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index = audio_buffer_index * F32_SIZE;
        audio_buffer[audio_buffer_index] = f32::from_be_bytes([audio_bytes_buffer[index],
                                            audio_bytes_buffer[index + 1],
                                            audio_bytes_buffer[index + 2],
                                            audio_bytes_buffer[index + 3]
                                            ]);
    }

    return audio_buffer;
}

fn write_bytes_buffer_to_file(mut writer: hound::WavWriter<io::BufWriter<fs::File>>, 
                                audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE], 
                                samples_to_write: usize,
                                sample_format: hound::SampleFormat,
                                sample_bytes_size: usize) -> io::Result<()> {
                                    
    if sample_format ==  hound::SampleFormat::Float {
        for audio_buffer_index in 0..samples_to_write {
            let index = audio_buffer_index * sample_bytes_size;
            let val = f32::from_be_bytes([audio_bytes_buffer[index],
                                audio_bytes_buffer[index + 1],
                                audio_bytes_buffer[index + 2],
                                audio_bytes_buffer[index + 3]
                                ]);
            writer.write_sample(val).unwrap();
        }
    }
    else {
        match sample_bytes_size {
            1 => {
                for audio_buffer_index in 0..samples_to_write {
                    let index = audio_buffer_index * sample_bytes_size;
                    let val = audio_bytes_buffer[index] as i8;
                    writer.write_sample(val).unwrap();
                }
            },
            2 => {
                for audio_buffer_index in 0..samples_to_write {
                    let index = audio_buffer_index * sample_bytes_size;
                    let val = ((audio_bytes_buffer[index] as i16) << 8)
                                    | (audio_bytes_buffer[index + 1] as i16);
                    writer.write_sample(val).unwrap();
                }
            },
            4 => {
                for audio_buffer_index in 0..samples_to_write {
                    let index = audio_buffer_index * sample_bytes_size;
                    let val = ((audio_bytes_buffer[index] as i32) << 24)
                                    | ((audio_bytes_buffer[index + 1] as i32) << 16)
                                    | ((audio_bytes_buffer[index + 2] as i32) << 8)
                                    | (audio_bytes_buffer[index + 3] as i32);
                    writer.write_sample(val).unwrap();
                }
            },
            _ => {panic!("Unsupported wav file format!")},
        }
    }

    Ok(())
}

fn tcp_receive_wav_file<P: AsRef<path::Path>>(stream: TcpStream, filename: P, wave_file_specs: WavFileSpecs) -> io::Result<()> {

    let mut samples_left : i64 = wave_file_specs.samples_amount as i64;
    let sample_bytes_size: usize = (wave_file_specs.bits_per_sample/8) as usize;
    let samples_per_buffer: usize = BYTE_BUFFER_SIZE / sample_bytes_size;

    let spec: hound::WavSpec = wav_spec_from_wav_file_specs(wave_file_specs)?;

    let mut writer: hound::WavWriter<io::BufWriter<fs::File>> = hound::WavWriter::create(filename, spec).unwrap();
    
    while  (samples_left - (samples_per_buffer as i64))> 0 {

        let audio_bytes_buffer = tcp_receive_bytes_chunk(&stream)?;

        //write_bytes_buffer_to_file(*caca,audio_bytes_buffer,samples_per_buffer,spec.sample_format,sample_bytes_size)?;
        
        if spec.sample_format ==  hound::SampleFormat::Float {
            let f32_buffer = bytes_buffer_to_f32(audio_bytes_buffer);
            for val in f32_buffer.into_iter() {
                writer.write_sample(val).unwrap();
            }
        }
        else {
            match sample_bytes_size {
                1 => {
                    let i8_buffer = bytes_buffer_to_i8(audio_bytes_buffer);
                    for val in i8_buffer.into_iter() {
                        writer.write_sample(val).unwrap();
                    }
                },
                2 => {
                    let i16_buffer = bytes_buffer_to_i16(audio_bytes_buffer);
                    for val in i16_buffer.into_iter() {
                        writer.write_sample(val).unwrap();
                    }
                },
                4 => {
                    let i32_buffer = bytes_buffer_to_i32(audio_bytes_buffer);
                    for val in i32_buffer.into_iter() {
                        writer.write_sample(val).unwrap();
                    }
                },
                _ => {panic!("Unsupported wav file format!")},
            }
        }

        samples_left = samples_left - (samples_per_buffer as i64);
    }
    //Process last batch if applicable
    if samples_left > 0 {
        let audio_bytes_buffer = tcp_receive_bytes_chunk(&stream)?;

        if spec.sample_format ==  hound::SampleFormat::Float {
            let f32_buffer = bytes_buffer_to_f32(audio_bytes_buffer);
            for i in 0..samples_left as usize {
                writer.write_sample(f32_buffer[i]).unwrap();
            }
        }
        else {
            match sample_bytes_size {
                1 => {
                    let i8_buffer = bytes_buffer_to_i8(audio_bytes_buffer);
                    for i in 0..samples_left as usize {
                        writer.write_sample(i8_buffer[i]).unwrap();
                    }
                },
                2 => {
                    let i16_buffer = bytes_buffer_to_i16(audio_bytes_buffer);
                    for i in 0..samples_left as usize {
                        writer.write_sample(i16_buffer[i]).unwrap();
                    }
                },
                4 => {
                    let i32_buffer = bytes_buffer_to_i32(audio_bytes_buffer);
                    for i in 0..samples_left as usize {
                        writer.write_sample(i32_buffer[i]).unwrap();
                    }
                },
                _ => {panic!("Unsupported wav file format!")},
            }
        }

        //write_bytes_buffer_to_file(writer,audio_bytes_buffer,(samples_left as usize),spec.sample_format,sample_bytes_size)?;
    }

    writer.finalize().unwrap();

    Ok(())
}

fn tcp_stream_to_vec(stream: TcpStream) -> std::io::Result<Vec<u8>>{
    let stream_clone: TcpStream = stream.try_clone()?;

    let mut data: Vec<u8> = Vec::new();
    let mut stream_buff: BufReader<_> = BufReader::new(stream_clone);

    let bytes_read: usize = stream_buff.read_until(b'}', &mut data)?;
    println!("Bytes read: {}", bytes_read);

    Ok(data)
}

fn tcp_stream_to_wav_file_specs(stream: TcpStream) -> std::io::Result<WavFileSpecs>{

    let json_raw_data: Vec<u8> = tcp_stream_to_vec(stream)?;

    let wav_file_specs: WavFileSpecs = serde_json::from_slice(&json_raw_data)?;

    Ok(wav_file_specs)
}

fn tcp_stream_to_files_list(stream: TcpStream) -> std::io::Result<FilesList>{

    let json_raw_data: Vec<u8> = tcp_stream_to_vec(stream)?;

    let files_list: FilesList = serde_json::from_slice(&json_raw_data)?;

    Ok(files_list)
}

fn tcp_send_client_request(stream: TcpStream, command: Command, filename: String) -> std::io::Result<()> {
    
    let client_request: ClientRequest = ClientRequest {
        command : command,
        filename : filename,
    };

    let client_request_string = serde_json::to_string(&client_request)?;
    println!("{}", client_request_string);

    to_writer(stream,&client_request)?;

    Ok(())
}

fn play_file() {
    let reader = hound::WavReader::open("target/received.wav").unwrap();
    let spec = reader.spec();

    // Read the interleaved samples and convert them to a signal.
    let samples = reader.into_samples::<i16>().filter_map(Result::ok);
    let mut frames = dasp_signal::from_interleaved_samples_iter(samples).until_exhausted();

    // Initialise CPAL.
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("failed to find a default output device");

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
    let stream = device.build_output_stream(&config, data_fn, err_fn, None).unwrap();
    stream.play().unwrap();

    // Block until playback completes.
    complete_rx.recv().unwrap();
    stream.pause().ok();
}

fn main() -> std::io::Result<()> {
    //play_file();
    //Get file list
    let stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    //let stream_clone = stream.try_clone()?;
    //tcp_send_client_request(stream, Command::PerformRecording,  "".to_string())?;

    //let stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    let stream_clone = stream.try_clone()?;
    tcp_send_client_request(stream_clone, Command::SendFilesList,  "".to_string())?;

    let stream_clone: TcpStream = stream.try_clone()?;
    let files_list: FilesList = tcp_stream_to_files_list(stream_clone)?;

    let files_list_string: String = serde_json::to_string(&files_list)?;
    println!("{}", files_list_string);

    //Choose file
    let filename : &String = &files_list.files[0];

    //Get file specs
    let stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    let stream_clone = stream.try_clone()?;
    tcp_send_client_request(stream_clone, Command::SendFileSpecs,  filename.to_string())?;

    let stream_clone = stream.try_clone()?;
    let wav_file_specs: WavFileSpecs = tcp_stream_to_wav_file_specs(stream_clone)?;

    let wav_file_specs_string: String = serde_json::to_string(&wav_file_specs)?;
    println!("{}", wav_file_specs_string);

    //Get file
    let stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    let stream_clone = stream.try_clone()?;
    tcp_send_client_request(stream_clone, Command::SendFile,  filename.to_string().to_string())?;

    let stream_clone = stream.try_clone()?;
    tcp_receive_wav_file(stream_clone,"target/received.wav",wav_file_specs)?;

    Ok(())
} // the stream is closed here
