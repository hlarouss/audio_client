use crate::common::resources::{ WavFileSpecs,FilesList,SampleFormat,Command,ClientRequest,
                                BYTE_BUFFER_SIZE,I8_SIZE,I16_SIZE,I32_SIZE,F32_SIZE,RECORD_DURATION};
use std::{  net::TcpStream,
            fs::File,
            path,
            io::{BufWriter, BufReader, BufRead, Read}};
use serde_json::to_writer;
use hound::{WavSpec,WavWriter};
use anyhow::Result;

fn vec(stream: TcpStream) -> Result<Vec<u8>>{
    let stream_clone: TcpStream = stream.try_clone()?;

    let mut data: Vec<u8> = Vec::new();
    let mut stream_buff: BufReader<_> = BufReader::new(stream_clone);

    let bytes_read: usize = stream_buff.read_until(b'}', &mut data)?;
    println!("Bytes read: {}", bytes_read);

    Ok(data)
}

fn send_client_request(stream: TcpStream, command: Command, filename: String, record_duration: u64) -> Result<()> {
    
    let client_request: ClientRequest = ClientRequest {
        command : command,
        filename : filename,
        record_duration : record_duration,
    };

    let client_request_string = serde_json::to_string(&client_request)?;
    println!("{}", client_request_string);

    to_writer(stream,&client_request)?;

    Ok(())
}

fn receive_wav_file_specs(stream: TcpStream, filename: String) -> Result<WavFileSpecs>{

    let stream_clone: TcpStream = stream.try_clone()?;
    send_client_request(stream_clone, Command::SendFileSpecs,  filename.to_string(),0)?;
    
    let json_raw_data: Vec<u8> = vec(stream)?;

    let wav_file_specs: WavFileSpecs = serde_json::from_slice(&json_raw_data)?;

    Ok(wav_file_specs)
}

pub fn receive_files_list(ip_address: &str) -> Result<FilesList>{

    let stream: TcpStream = TcpStream::connect(ip_address)?;

    let stream_clone: TcpStream = stream.try_clone()?;
    send_client_request(stream_clone, Command::SendFilesList,  "".to_string(),0)?;

    let stream_clone: TcpStream = stream.try_clone()?;
    let json_raw_data: Vec<u8> = vec(stream_clone)?;

    let files_list: FilesList = serde_json::from_slice(&json_raw_data)?;

    Ok(files_list)
}

pub fn perform_recording(ip_address: &str) -> Result<()>{

    let stream: TcpStream = TcpStream::connect(ip_address)?;

    let stream_clone: TcpStream = stream.try_clone()?;
    send_client_request(stream_clone, Command::PerformRecording,  "".to_string(),RECORD_DURATION)?;

    Ok(())
}

fn wav_spec(wav_file_specs: WavFileSpecs) -> Result<WavSpec> {

    let sample_format: hound::SampleFormat = match wav_file_specs.sample_format {
        SampleFormat::Float => hound::SampleFormat::Float,
        SampleFormat::Int => hound::SampleFormat::Int,
    };

    Ok(WavSpec {
        channels: wav_file_specs.channels,
        sample_rate: wav_file_specs.sample_rate,
        bits_per_sample: wav_file_specs.bits_per_sample,
        sample_format: sample_format,
    })
}

fn receive_bytes_chunk(mut stream: TcpStream) -> Result<[u8;BYTE_BUFFER_SIZE]>{
    let mut audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE] = [0;BYTE_BUFFER_SIZE];

    stream.read_exact(&mut audio_bytes_buffer)?;

    Ok(audio_bytes_buffer)
}

fn audio_buffer_i8(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [i8;BYTE_BUFFER_SIZE]{
    let mut audio_buffer : [i8;BYTE_BUFFER_SIZE/I8_SIZE] = [0;BYTE_BUFFER_SIZE/I8_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index: usize = audio_buffer_index * I8_SIZE;
        audio_buffer[audio_buffer_index]  = audio_bytes_buffer[index] as i8;
    }

    return audio_buffer;
}

fn audio_buffer_i16(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [i16;BYTE_BUFFER_SIZE/I16_SIZE]{
    let mut audio_buffer : [i16;BYTE_BUFFER_SIZE/I16_SIZE] = [0;BYTE_BUFFER_SIZE/I16_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index: usize = audio_buffer_index * I16_SIZE;
        audio_buffer[audio_buffer_index] = ((audio_bytes_buffer[index] as i16) << 8)
                                            | (audio_bytes_buffer[index + 1] as i16);
    }

    return audio_buffer;
}

fn audio_buffer_i32(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [i32;BYTE_BUFFER_SIZE/I32_SIZE]{
    let mut audio_buffer : [i32;BYTE_BUFFER_SIZE/I32_SIZE] = [0;BYTE_BUFFER_SIZE/I32_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index: usize = audio_buffer_index * I32_SIZE;
        audio_buffer[audio_buffer_index] = ((audio_bytes_buffer[index] as i32) << 24)
                                            | ((audio_bytes_buffer[index + 1] as i32) << 16)
                                            | ((audio_bytes_buffer[index + 2] as i32) << 8)
                                            | (audio_bytes_buffer[index + 3] as i32);
    }

    return audio_buffer;
}

fn audio_buffer_f32(audio_bytes_buffer: [u8;BYTE_BUFFER_SIZE]) -> [f32;BYTE_BUFFER_SIZE/F32_SIZE]{
    let mut audio_buffer : [f32;BYTE_BUFFER_SIZE/F32_SIZE] = [0.0;BYTE_BUFFER_SIZE/F32_SIZE];

    for audio_buffer_index in 0..audio_buffer.len() {
        let index: usize = audio_buffer_index * F32_SIZE;
        audio_buffer[audio_buffer_index] = f32::from_be_bytes([audio_bytes_buffer[index],
                                            audio_bytes_buffer[index + 1],
                                            audio_bytes_buffer[index + 2],
                                            audio_bytes_buffer[index + 3]
                                            ]);
    }

    return audio_buffer;
}

pub fn receive_wav_file<P: AsRef<path::Path>>(ip_address: &str, requested_file: String,filename: P) -> Result<()> {

    //filename= Destination file path on client computer

    let requested_file_copy = requested_file.clone();
    let stream: TcpStream = TcpStream::connect(ip_address)?;
    let wave_file_specs: WavFileSpecs = receive_wav_file_specs(stream,requested_file_copy)?;
    
    let mut samples_left : i64 = wave_file_specs.samples_amount as i64;
    let sample_bytes_size: usize = (wave_file_specs.bits_per_sample/8) as usize;
    let samples_per_buffer: usize = BYTE_BUFFER_SIZE / sample_bytes_size;

    let spec: WavSpec = wav_spec(wave_file_specs)?;

    let mut writer: WavWriter<BufWriter<File>> = WavWriter::create(filename, spec)?;

    let requested_file_copy = requested_file.clone();
    let stream: TcpStream = TcpStream::connect(ip_address)?;
    let stream_clone: TcpStream = stream.try_clone()?;
    send_client_request(stream_clone, Command::SendFile,  requested_file_copy,0)?;

    while samples_left > 0 {
        let stream_clone: TcpStream = stream.try_clone()?;
        let audio_bytes_buffer: [u8; BYTE_BUFFER_SIZE] = receive_bytes_chunk(stream_clone)?;

        let samples_to_write: usize = if (samples_left - (samples_per_buffer as i64))> 0 {samples_per_buffer} else {samples_left as usize};

        if spec.sample_format ==  hound::SampleFormat::Float {
            let audio_samples_buffer = audio_buffer_f32(audio_bytes_buffer);
            for audio_sample_index in 0..samples_to_write as usize {
                writer.write_sample(audio_samples_buffer[audio_sample_index])?;
            }
        }
        else {
            match sample_bytes_size {
                I8_SIZE => {
                    let audio_samples_buffer = audio_buffer_i8(audio_bytes_buffer);
                    for audio_sample_index in 0..samples_to_write as usize {
                        writer.write_sample(audio_samples_buffer[audio_sample_index])?;
                    }
                },
                I16_SIZE => {
                    let audio_samples_buffer = audio_buffer_i16(audio_bytes_buffer);
                    for audio_sample_index in 0..samples_to_write as usize {
                        writer.write_sample(audio_samples_buffer[audio_sample_index])?;
                    }
                },
                I32_SIZE => {
                    let audio_samples_buffer = audio_buffer_i32(audio_bytes_buffer);
                    for audio_sample_index in 0..samples_to_write as usize {
                        writer.write_sample(audio_samples_buffer[audio_sample_index])?;
                    }
                },
                byte_size => {return Err(anyhow::Error::msg(format!(
                    "Unsupported sample byte size'{byte_size}'"
                    )))},
            }
        }

        samples_left = samples_left - (samples_to_write as i64);

    }
    
    writer.finalize()?;

    Ok(())
}





