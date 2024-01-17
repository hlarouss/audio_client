use std::io::prelude::*;
use std::net::TcpStream;
use std::io;
use std::path;
use std::fs;
use hound;
use serde_json::to_writer;
use std::io::BufReader;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Command {
    PerformRecording,
    SendFilesList,
    SendFileSpecs,
    SendFile,
}
#[derive(Serialize, Deserialize)]
struct WavSpecs {
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    samples_amount: u32
}

#[derive(Serialize, Deserialize)]
struct ClientRequest {
    command: Command,
    filename: String,
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

fn tcp_receive_wav_file<P: AsRef<path::Path>>(stream: &TcpStream, filename: P, wave_file_specs: WavSpecs) -> io::Result<()> {

    let mut samples_left : i64 = wave_file_specs.samples_amount as i64;

    let spec: hound::WavSpec = hound::WavSpec {
        channels: wave_file_specs.channels,
        sample_rate: wave_file_specs.sample_rate,
        bits_per_sample: wave_file_specs.bits_per_sample,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer: hound::WavWriter<io::BufWriter<fs::File>> = hound::WavWriter::create(filename, spec).unwrap();

    while  samples_left - (i16::MAX as i64) > 0 {
        let audio_buffer: [i16;i16::MAX as usize] = tcp_receive_chunk(&stream)?;

        for val in audio_buffer.into_iter() {
            writer.write_sample(val).unwrap();
        }

        samples_left = samples_left - (i16::MAX as i64);
    }
    //Process last batch if applicable
    if samples_left > 0 {
        let audio_buffer: [i16;i16::MAX as usize] = tcp_receive_chunk(&stream)?;

        for i in 0..samples_left as usize {
            writer.write_sample(audio_buffer[i]).unwrap();
        }
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

fn tcp_stream_to_wav_file_specs(stream: TcpStream) -> std::io::Result<WavSpecs>{

    let json_raw_data: Vec<u8> = tcp_stream_to_vec(stream)?;

    let wav_file_specs: WavSpecs = serde_json::from_slice(&json_raw_data)?;

    Ok(wav_file_specs)
}

fn main() -> std::io::Result<()> {
    let stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    //Get file specs
    let client_request: ClientRequest = ClientRequest {
        command : Command::SendFileSpecs,
        filename : "target/prerecorded2.wav".to_string(),
    };

    let specs_string = serde_json::to_string(&client_request)?;
    println!("{}", specs_string);

    let stream_clone = stream.try_clone().expect("clone failed...");

    to_writer(stream_clone,&client_request)?;

    let stream_clone: TcpStream = stream.try_clone()?;
    let wav_file_specs: WavSpecs = tcp_stream_to_wav_file_specs(stream_clone)?;


    let wav_file_specs_string: String = serde_json::to_string(&wav_file_specs)?;
    println!("{}", wav_file_specs_string);

    let stream: TcpStream = TcpStream::connect("127.0.0.1:8000")?;

    //Get file
    let client_request: ClientRequest = ClientRequest {
        command : Command::SendFile,
        filename : "target/prerecorded2.wav".to_string(),
    };

    to_writer(&stream,&client_request)?;

    tcp_receive_wav_file(&stream,client_request.filename,wav_file_specs)?;

    Ok(())
} // the stream is closed here
